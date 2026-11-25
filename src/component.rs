use std::{
    any::{Any, TypeId},
    cell::UnsafeCell,
    collections::HashMap,
};

use crate::{
    bundle::Bundle,
    entity::Entity,
    sparse_set::{SparseSet, SparseSetRawParts, SparseSetRawPartsMut},
};

trait Storage: Any {
    fn remove_components(&mut self, entity: Entity);
}

impl<T: Component> Storage for SparseSet<T> {
    fn remove_components(&mut self, entity: Entity) {
        self.remove(entity);
    }
}

#[derive(Default)]
pub struct ComponentRegistry {
    storages: HashMap<TypeId, UnsafeCell<Box<dyn Storage>>>,
}

impl ComponentRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn insert<T: Component>(&mut self, entity: Entity, component: T) -> Option<T> {
        if let Some(storage) = self.get_storage_mut::<T>() {
            storage.insert(entity, component)
        } else {
            let mut storage = SparseSet::<T>::new();
            storage.insert(entity, component);

            let type_id = TypeId::of::<T>();
            self.storages
                .insert(type_id, UnsafeCell::new(Box::new(storage)));

            None
        }
    }

    pub fn get<T: Component>(&self, entity: Entity) -> Option<&T> {
        self.get_storage::<T>()?.get(entity)
    }

    pub fn get_mut<T: Component>(&mut self, entity: Entity) -> Option<&mut T> {
        self.get_storage_mut::<T>()?.get_mut(entity)
    }

    pub fn remove<T: Component>(&mut self, entity: Entity) -> Option<T> {
        self.get_storage_mut::<T>()?.remove(entity)
    }

    fn contains<T: Component>(&self, entity: Entity) -> bool {
        self.get::<T>(entity).is_some()
    }

    fn get_storage<T: Component>(&self) -> Option<&SparseSet<T>> {
        let type_id = TypeId::of::<T>();
        let storage = self.storages.get(&type_id)?;
        // SAFETY:
        // 调用方必须保证不存在对该 Storage 的可变访问。
        let storage = unsafe { &*storage.get() }.as_ref();
        (storage as &dyn Any).downcast_ref::<SparseSet<T>>()
    }

    fn get_storage_mut<T: Component>(&mut self) -> Option<&mut SparseSet<T>> {
        let type_id = TypeId::of::<T>();
        let storage = self.storages.get_mut(&type_id)?;
        let storage = storage.get_mut().as_mut();
        (storage as &mut dyn Any).downcast_mut::<SparseSet<T>>()
    }

    pub fn remove_components(&mut self, entity: Entity) {
        for storage in self.storages.values_mut() {
            storage.get_mut().remove_components(entity);
        }
    }

    pub(crate) fn raw_parts<T: Component>(&self) -> Option<SparseSetRawParts<'_, T>> {
        Some(self.get_storage::<T>()?.raw_parts())
    }

    /// # Safety
    ///
    /// 调用方必须保证该 storage 在返回的 raw parts 存活期间没有重叠的共享或可变访问，
    /// 且不会发生结构性修改。
    pub(crate) unsafe fn raw_parts_mut<T: Component>(&self) -> Option<SparseSetRawPartsMut<'_, T>> {
        let type_id = TypeId::of::<T>();
        let storage = self.storages.get(&type_id)?;
        let storage = unsafe { &mut *storage.get() }.as_mut();
        let storage = (storage as &mut dyn Any).downcast_mut::<SparseSet<T>>()?;
        Some(storage.raw_parts_mut())
    }
}

pub trait Component: 'static {}

impl<T: Component> Bundle for T {
    fn insert(self, entity: Entity, registry: &mut ComponentRegistry) {
        registry.insert(entity, self);
    }
}

#[cfg(test)]
mod tests {
    use crate::Component;

    use super::*;
    use std::assert_matches;

    #[derive(PartialEq, Debug, Component)]
    struct Position {
        x: f32,
        y: f32,
    }

    #[derive(PartialEq, Debug, Component)]
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

    #[test]
    fn insert_returns_none_when_new_component() {
        let mut registry = ComponentRegistry::new();
        let entity = Entity::new(0, 0);

        let old = registry.insert(entity, Position { x: 1.0, y: 2.0 });

        assert!(old.is_none());
    }

    #[test]
    fn insert_returns_old_component_when_replaced() {
        let mut registry = ComponentRegistry::new();
        let entity = Entity::new(0, 0);

        registry.insert(entity, Position { x: 1.0, y: 2.0 });

        let old = registry.insert(entity, Position { x: 2.0, y: 3.0 });

        assert_eq!(old.unwrap().x, 1.0);
    }

    #[test]
    fn remove_components_clears_all_types_for_entity_only() {
        let mut registry = ComponentRegistry::new();
        let entity = Entity::new(1, 0);
        let other_entity = Entity::new(2, 0);

        registry.insert(entity, Position { x: 1.0, y: 2.0 });
        registry.insert(entity, Health(100));
        registry.insert(other_entity, Position { x: 3.0, y: 4.0 });
        registry.insert(other_entity, Health(50));

        registry.remove_components(entity);

        assert_matches!(registry.get::<Position>(entity), None);
        assert_matches!(registry.get::<Health>(entity), None);
        assert_eq!(
            registry.get::<Position>(other_entity),
            Some(&Position { x: 3.0, y: 4.0 })
        );
        assert_eq!(registry.get::<Health>(other_entity), Some(&Health(50)));
    }

    #[test]
    fn remove_components_ignores_stale_generation() {
        let mut registry = ComponentRegistry::new();
        let stale_entity = Entity::new(1, 0);
        let current_entity = Entity::new(1, 1);

        registry.insert(current_entity, Position { x: 1.0, y: 2.0 });
        registry.insert(current_entity, Health(100));

        registry.remove_components(stale_entity);

        assert_eq!(
            registry.get::<Position>(current_entity),
            Some(&Position { x: 1.0, y: 2.0 })
        );
        assert_eq!(registry.get::<Health>(current_entity), Some(&Health(100)));
    }
}
