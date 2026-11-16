use std::{
    any::{Any, TypeId},
    collections::HashMap,
};

use crate::{entity::Entity, sparse_set::SparseSet};

struct ComponentRegistry {
    storages: HashMap<TypeId, Box<dyn Any>>,
}

impl ComponentRegistry {
    fn new() -> Self {
        Self {
            storages: HashMap::new(),
        }
    }

    fn insert<T: 'static>(&mut self, entity: Entity, component: T) {
        if let Some(storage) = self.get_storage_mut::<T>() {
            storage.insert(entity, component);
        } else {
            let mut storage = SparseSet::<T>::new();
            storage.insert(entity, component);

            let typeid = TypeId::of::<T>();
            self.storages.insert(typeid, Box::new(storage));
        }
    }

    fn get<T: 'static>(&self, entity: Entity) -> Option<&T> {
        self.get_storage::<T>()?.get(entity)
    }

    fn get_mut<T: 'static>(&mut self, entity: Entity) -> Option<&mut T> {
        self.get_storage_mut::<T>()?.get_mut(entity)
    }

    fn remove<T: 'static>(&mut self, entity: Entity) -> Option<T> {
        self.get_storage_mut::<T>()?.remove(entity)
    }

    fn contains<T: 'static>(&self, entity: Entity) -> bool {
        self.get::<T>(entity).is_some()
    }

    fn get_storage<T: 'static>(&self) -> Option<&SparseSet<T>> {
        let typeid = TypeId::of::<T>();
        self.storages
            .get(&typeid)
            .map(|s| s.downcast_ref::<SparseSet<T>>())?
    }

    fn get_storage_mut<T: 'static>(&mut self) -> Option<&mut SparseSet<T>> {
        let typeid = TypeId::of::<T>();
        self.storages
            .get_mut(&typeid)
            .map(|s| s.downcast_mut::<SparseSet<T>>())?
    }
}


#[cfg(test)]
mod tests {
    use super::*;

    #[derive(PartialEq, Debug)]
    struct Position {
        x: f32,
        y: f32,
    }

    #[derive(PartialEq, Debug)]
    struct Health(u32);

    #[test]
    fn test_insert_and_get() {
        let mut registry = ComponentRegistry::new();
        let entity = Entity::new(1, 0);

        registry.insert(entity, Position { x: 1.0, y: 2.0 });

        assert_eq!(
            registry.get::<Position>(entity),
            Some(&Position { x: 1.0, y: 2.0 })
        );
    }

    #[test]
    fn test_multiple_component_types() {
        let mut registry = ComponentRegistry::new();
        let entity = Entity::new(1, 0);

        registry.insert(entity, Position { x: 1.0, y: 2.0 });
        registry.insert(entity, Health(100));

        assert_eq!(
            registry.get::<Position>(entity),
            Some(&Position { x: 1.0, y: 2.0 })
        );
        assert_eq!(registry.get::<Health>(entity), Some(&Health(100)));
    }

    #[test]
    fn test_multiple_entities() {
        let mut registry = ComponentRegistry::new();

        let entity1 = Entity::new(1, 0);
        let entity2 = Entity::new(2, 0);

        registry.insert(entity1, Position { x: 1.0, y: 2.0 });
        registry.insert(entity2, Position { x: 3.0, y: 4.0 });

        assert_eq!(
            registry.get::<Position>(entity1),
            Some(&Position { x: 1.0, y: 2.0 })
        );
        assert_eq!(
            registry.get::<Position>(entity2),
            Some(&Position { x: 3.0, y: 4.0 })
        );
    }

    #[test]
    fn test_replace() {
        let mut registry = ComponentRegistry::new();
        let entity = Entity::new(1, 0);

        registry.insert(entity, Position { x: 1.0, y: 2.0 });
        registry.insert(entity, Position { x: 3.0, y: 4.0 });

        assert_eq!(
            registry.get::<Position>(entity),
            Some(&Position { x: 3.0, y: 4.0 })
        );
    }

    #[test]
    fn test_get_mut() {
        let mut registry = ComponentRegistry::new();
        let entity = Entity::new(1, 0);

        registry.insert(entity, Health(100));

        registry.get_mut::<Health>(entity).unwrap().0 = 50;

        assert_eq!(registry.get::<Health>(entity), Some(&Health(50)));
    }

    #[test]
    fn test_remove() {
        let mut registry = ComponentRegistry::new();
        let entity = Entity::new(1, 0);

        registry.insert(entity, Position { x: 1.0, y: 2.0 });

        assert_eq!(
            registry.remove::<Position>(entity),
            Some(Position { x: 1.0, y: 2.0 })
        );

        assert_eq!(registry.get::<Position>(entity), None);
    }

    #[test]
    fn test_get_missing() {
        let registry = ComponentRegistry::new();
        let entity = Entity::new(1, 0);

        assert_eq!(registry.get::<Position>(entity), None);
        assert_eq!(registry.get::<Health>(entity), None);
    }

    #[test]
    fn test_generation_validation() {
        let mut registry = ComponentRegistry::new();

        let old_entity = Entity::new(1, 0);
        let new_entity = Entity::new(1, 1);

        registry.insert(old_entity, Position { x: 1.0, y: 2.0 });

        assert_eq!(registry.get::<Position>(new_entity), None);
        assert!(!registry.contains::<Position>(new_entity));
    }

    #[test]
    fn test_empty_storage_retained() {
        let mut registry = ComponentRegistry::new();
        let entity = Entity::new(1, 0);

        registry.insert(entity, Position { x: 1.0, y: 2.0 });
        registry.remove::<Position>(entity);

        assert!(registry.storages.contains_key(&TypeId::of::<Position>()));
        assert!(registry.get_storage::<Position>().unwrap().is_empty());
    }
}
