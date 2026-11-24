use std::marker::PhantomData;

use crate::{
    access::validate_access_or_panic,
    bundle::Bundle,
    component::{Component, ComponentRegistry},
    entity::{Entities, Entity},
    error::{RecsError, RecsResult},
    query::{Query, QueryData},
    sparse_set::SparseSetRawParts,
    system::{IntoSystem, System},
};

#[derive(Default)]
pub struct World {
    entities: Entities,
    pub(crate) components: ComponentRegistry,
}

impl World {
    pub fn new() -> Self {
        Self::default()
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

    pub fn insert<B: Bundle>(&mut self, entity: Entity, bundle: B) -> RecsResult<()> {
        self.check(entity)?;
        bundle.insert(entity, &mut self.components);
        Ok(())
    }

    pub fn remove<T: Component>(&mut self, entity: Entity) -> RecsResult<Option<T>> {
        self.check(entity)?;
        Ok(self.components.remove(entity))
    }

    pub fn get<T: Component>(&self, entity: Entity) -> RecsResult<Option<&T>> {
        self.check(entity)?;
        Ok(self.components.get(entity))
    }

    pub fn get_mut<T: Component>(&mut self, entity: Entity) -> RecsResult<Option<&mut T>> {
        self.check(entity)?;
        Ok(self.components.get_mut(entity))
    }

    pub fn query<Q: QueryData>(&mut self) -> Query<'_, Q> {
        validate_access_or_panic(Q::init_access);
        let cell = WorldCell::new(self);
        // SAFETY: `QueryData` 的`init_access()` 已保证通过冲突检查
        unsafe { Query::init(&cell) }
    }

    pub fn run_system<M>(&mut self, system: impl IntoSystem<M>) {
        let mut system = system.into_system();
        let cell = WorldCell::new(self);
        system.run(&cell);
    }
}

pub struct WorldCell<'w> {
    world: *mut World,
    _marker: PhantomData<&'w mut World>,
}

impl<'w> WorldCell<'w> {
    pub fn new(world: &'w mut World) -> Self {
        Self {
            world,
            _marker: PhantomData,
        }
    }

    pub unsafe fn raw_parts_mut<T: Component>(&self) -> Option<SparseSetRawParts<'w, T>> {
        let world = unsafe { &mut *self.world };
        world.components.raw_parts_mut::<T>()
    }
}
#[cfg(test)]
mod tests {
    use crate::Component;

    use super::*;
    use std::assert_matches;

    #[derive(Debug, PartialEq, Component)]
    struct Position(i32, i32);

    #[derive(Debug, PartialEq, Component)]
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
    fn spawn_inserts_single_component_bundle() {
        let mut world = World::new();

        let entity = world.spawn(Position(3, 5));

        assert_matches!(world.get::<Position>(entity), Ok(Some(Position(3, 5))));
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
