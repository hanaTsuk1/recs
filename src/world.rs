use crate::{
    bundle::Bundle,
    component::ComponentRegistry,
    entity::{Entities, Entity},
    error::{RecsError, RecsResult},
};

pub struct World {
    entities: Entities,
    components: ComponentRegistry,
}

impl World {
    pub fn new() -> Self {
        Self {
            entities: Entities::new(),
            components: ComponentRegistry::new(),
        }
    }

    fn check(&self, entity: Entity) -> RecsResult<()> {
        self.entities
            .contains(entity)
            .ok_or(RecsError::NoSuchEntity)
    }

    pub fn spawn<B: Bundle>(&mut self, bundle: B) -> Entity {
        let entity = self.entities.spawn();
        bundle.insert(entity, &mut self.components);
        entity
    }

    pub fn despawn(&mut self, entity: Entity) -> RecsResult<()> {
        if self.entities.despawn(entity) {
            self.components.remove_components(entity);
            Ok(())
        } else {
            Err(RecsError::NoSuchEntity)
        }
    }

    pub fn insert<T: 'static>(&mut self, entity: Entity, component: T) -> RecsResult<Option<T>> {
        self.check(entity)?;
        Ok(self.components.insert(entity, component))
    }

    pub fn remove<T: 'static>(&mut self, entity: Entity) -> RecsResult<Option<T>> {
        self.check(entity)?;
        Ok(self.components.remove(entity))
    }

    pub fn get<T: 'static>(&self, entity: Entity) -> RecsResult<Option<&T>> {
        self.check(entity)?;
        Ok(self.components.get(entity))
    }

    pub fn get_mut<T: 'static>(&mut self, entity: Entity) -> RecsResult<Option<&mut T>> {
        self.check(entity)?;
        Ok(self.components.get_mut(entity))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::assert_matches;

    #[derive(Debug, PartialEq)]
    struct Position(i32, i32);

    #[derive(Debug, PartialEq)]
    struct Health(u32);

    struct ActorBundle {
        position: Position,
        health: Health,
    }

    impl Bundle for ActorBundle {
        fn insert(self, entity: Entity, registry: &mut ComponentRegistry) {
            registry.insert(entity, self.position);
            registry.insert(entity, self.health);
        }
    }

    #[test]
    fn spawn_inserts_tuple_bundle_components() {
        let mut world = World::new();
        let entity = world.spawn((Position(3, 5), Health(100)));

        assert_matches!(world.get::<Position>(entity), Ok(Some(Position(3, 5))));
        assert_matches!(world.get::<Health>(entity), Ok(Some(Health(100))));
    }

    #[test]
    fn spawn_inserts_user_defined_struct_bundle_components() {
        let mut world = World::new();
        let entity = world.spawn(ActorBundle {
            position: Position(3, 5),
            health: Health(100),
        });

        assert_matches!(world.get::<Position>(entity), Ok(Some(Position(3, 5))));
        assert_matches!(world.get::<Health>(entity), Ok(Some(Health(100))));
    }

    #[test]
    fn insert_returns_replaced_component() {
        let mut world = World::new();
        let entity = world.spawn((Position(1, 2),));

        assert_matches!(
            world.insert(entity, Position(8, 9)),
            Ok(Some(Position(1, 2)))
        );
        assert_matches!(world.get::<Position>(entity), Ok(Some(Position(8, 9))));
    }

    #[test]
    fn get_mut_and_remove_work_for_live_entity() {
        let mut world = World::new();
        let entity = world.spawn((Health(100),));

        if let Ok(Some(health)) = world.get_mut::<Health>(entity) {
            health.0 = 75;
        } else {
            panic!("expected health component");
        }

        assert_matches!(world.remove::<Health>(entity), Ok(Some(Health(75))));
        assert_matches!(world.get::<Health>(entity), Ok(None));
        assert_matches!(world.remove::<Health>(entity), Ok(None));
    }

    #[test]
    fn despawn_cleans_components_and_rejects_stale_handle() {
        let mut world = World::new();
        let entity = world.spawn((Position(3, 5), Health(100)));

        assert_matches!(world.despawn(entity), Ok(()));
        assert_matches!(world.despawn(entity), Err(RecsError::NoSuchEntity));

        let reused = world.spawn(());
        assert_eq!(entity.index(), reused.index());
        assert_ne!(entity, reused);

        assert_matches!(world.get::<Position>(entity), Err(RecsError::NoSuchEntity));
        assert_matches!(
            world.insert(entity, Position(8, 9)),
            Err(RecsError::NoSuchEntity)
        );
        assert_matches!(world.get::<Position>(reused), Ok(None));
    }

    #[test]
    fn component_operations_reject_unknown_entity() {
        let mut world = World::new();
        let entity = Entity::new(999, 0);

        assert_matches!(
            world.insert(entity, Position(1, 2)),
            Err(RecsError::NoSuchEntity)
        );
        assert_matches!(world.get::<Position>(entity), Err(RecsError::NoSuchEntity));
        assert_matches!(
            world.get_mut::<Position>(entity),
            Err(RecsError::NoSuchEntity)
        );
        assert_matches!(
            world.remove::<Position>(entity),
            Err(RecsError::NoSuchEntity)
        );
        assert_matches!(world.despawn(entity), Err(RecsError::NoSuchEntity));
    }
}
