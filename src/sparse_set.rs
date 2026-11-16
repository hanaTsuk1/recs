use crate::entity::Entity;

struct SparseSet<T> {
    sparse: Vec<Option<usize>>,
    entities: Vec<Entity>,
    dense: Vec<T>,
}

impl<T> SparseSet<T> {
    fn new() -> Self {
        Self {
            sparse: vec![],
            entities: vec![],
            dense: vec![],
        }
    }

    fn len(&self) -> usize {
        self.entities.len()
    }

    fn is_empty(&self) -> bool {
        self.entities.is_empty()
    }

    fn get_dense_index(&self, entity: Entity) -> Option<usize> {
        let dense_index = self.sparse.get(entity.index() as usize)?.as_ref()?;

        if self.entities.get(*dense_index) == Some(&entity) {
            Some(*dense_index)
        } else {
            None
        }
    }

    fn insert(&mut self, entity: Entity, component: T) -> Option<T> {
        if let Some(dense_index) = self.get_dense_index(entity) {
            Some(std::mem::replace(&mut self.dense[dense_index], component))
        } else {
            self.entities.push(entity);
            self.dense.push(component);
            let entity_index = entity.index() as usize;
            if entity_index >= self.sparse.len() {
                self.sparse.resize(entity_index + 1, None);
            }
            let last_index = self.len() - 1;
            self.sparse[entity_index] = Some(last_index);
            None
        }
    }

    fn get(&self, entity: Entity) -> Option<&T> {
        let index = self.get_dense_index(entity)?;
        self.dense.get(index)
    }

    fn get_mut(&mut self, entity: Entity) -> Option<&mut T> {
        let index = self.get_dense_index(entity)?;
        self.dense.get_mut(index)
    }

    fn remove(&mut self, entity: Entity) -> Option<T> {
        let index = self.get_dense_index(entity)?;

        self.sparse[entity.index() as usize] = None;

        self.entities.swap_remove(index);
        let component = self.dense.swap_remove(index);

        if let Some(moved_entity) = self.entities.get(index) {
            self.sparse[moved_entity.index() as usize] = Some(index);
        }

        Some(component)
    }

    fn contains(&self, entity: Entity) -> bool {
        self.get(entity).is_some()
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

    #[test]
    fn test_insert() {
        let mut storage = SparseSet::new();

        let e1 = Entity::new(1, 0);

        storage.insert(e1, Position { x: 10.0, y: 20.0 });

        assert_eq!(storage.len(), 1);
        assert!(storage.contains(e1));

        assert_eq!(storage.get(e1), Some(&Position { x: 10.0, y: 20.0 }));
    }

    #[test]
    fn test_insert_multiple() {
        let mut storage = SparseSet::new();

        let e1 = Entity::new(1, 0);
        let e2 = Entity::new(2, 0);
        let e3 = Entity::new(3, 0);

        storage.insert(e1, Position { x: 1.0, y: 1.0 });
        storage.insert(e2, Position { x: 2.0, y: 2.0 });
        storage.insert(e3, Position { x: 3.0, y: 3.0 });

        assert_eq!(storage.len(), 3);

        assert!(storage.contains(e1));
        assert!(storage.contains(e2));
        assert!(storage.contains(e3));
    }

    #[test]
    fn test_get_nonexistent() {
        let storage = SparseSet::<Position>::new();

        let e1 = Entity::new(1, 0);

        assert_eq!(storage.get(e1), None);
        assert!(!storage.contains(e1));
    }

    #[test]
    fn test_get_mut() {
        let mut storage = SparseSet::new();

        let e1 = Entity::new(1, 0);

        storage.insert(e1, Position { x: 1.0, y: 2.0 });

        let position = storage.get_mut(e1).unwrap();
        position.x = 100.0;

        assert_eq!(storage.get(e1), Some(&Position { x: 100.0, y: 2.0 }));
    }

    #[test]
    fn test_remove() {
        let mut storage = SparseSet::new();

        let e1 = Entity::new(1, 0);

        storage.insert(e1, Position { x: 1.0, y: 2.0 });

        let removed = storage.remove(e1);

        assert_eq!(removed, Some(Position { x: 1.0, y: 2.0 }));

        assert_eq!(storage.len(), 0);
        assert!(!storage.contains(e1));
        assert_eq!(storage.get(e1), None);
    }

    #[test]
    fn test_remove_nonexistent() {
        let mut storage = SparseSet::<Position>::new();

        let e1 = Entity::new(1, 0);

        assert_eq!(storage.remove(e1), None);
        assert_eq!(storage.len(), 0);
    }

    #[test]
    fn test_remove_middle() {
        let mut storage = SparseSet::new();

        let e1 = Entity::new(1, 0);
        let e2 = Entity::new(2, 0);
        let e3 = Entity::new(3, 0);

        storage.insert(e1, Position { x: 1.0, y: 1.0 });
        storage.insert(e2, Position { x: 2.0, y: 2.0 });
        storage.insert(e3, Position { x: 3.0, y: 3.0 });

        storage.remove(e2);

        assert_eq!(storage.len(), 2);

        assert_eq!(storage.get(e1), Some(&Position { x: 1.0, y: 1.0 }));

        assert_eq!(storage.get(e3), Some(&Position { x: 3.0, y: 3.0 }));

        assert!(!storage.contains(e2));
    }

    #[test]
    fn test_generation_check() {
        let mut storage = SparseSet::new();

        let old = Entity::new(1, 0);
        let new = Entity::new(1, 1);

        storage.insert(old, Position { x: 10.0, y: 20.0 });

        assert!(storage.contains(old));
        assert!(!storage.contains(new));

        assert_eq!(storage.get(new), None);
    }

    #[test]
    fn test_reused_entity_id() {
        let mut storage = SparseSet::new();

        let old = Entity::new(1, 0);
        let new = Entity::new(1, 1);

        storage.insert(old, Position { x: 10.0, y: 20.0 });

        storage.remove(old);

        storage.insert(new, Position { x: 30.0, y: 40.0 });

        assert_eq!(storage.get(old), None);

        assert_eq!(storage.get(new), Some(&Position { x: 30.0, y: 40.0 }));
    }

    #[test]
    fn test_stale_entity_cannot_remove_new_component() {
        let mut storage = SparseSet::new();

        let old = Entity::new(1, 0);
        let new = Entity::new(1, 1);

        storage.insert(new, Position { x: 30.0, y: 40.0 });

        assert_eq!(storage.remove(old), None);

        assert!(storage.contains(new));
    }

    #[test]
    fn test_insert_existing_entity() {
        let mut storage = SparseSet::new();

        let e1 = Entity::new(1, 0);

        storage.insert(e1, Position { x: 1.0, y: 1.0 });

        let old = storage.insert(e1, Position { x: 2.0, y: 2.0 });

        assert_eq!(old, Some(Position { x: 1.0, y: 1.0 }));

        assert_eq!(storage.len(), 1);

        assert_eq!(storage.get(e1), Some(&Position { x: 2.0, y: 2.0 }));
    }
}
