#[derive(PartialEq, Debug, Clone, Copy)]
struct Entity {
    index: u32,
    generation: u32,
}

#[derive(Default)]
struct Entities {
    slots: Vec<EntitySlot>,
    free: Vec<u32>,
}

struct EntitySlot {
    generation: u32,
    state: EntityState,
}

#[derive(PartialEq)]
enum EntityState {
    Alive,
    Free,
}

impl Entities {
    fn new() -> Self {
        Self::default()
    }

    fn spawn(&mut self) -> Entity {
        if let Some(slot_id) = self.free.pop() {
            let slot = &mut self.slots[slot_id as usize];
            slot.state = EntityState::Alive;

            Entity {
                index: slot_id,
                generation: slot.generation,
            }
        } else {
            let index = u32::try_from(self.slots.len()).expect("too many entities");
            let entity = Entity { index, generation: 0 };

            self.slots.push(EntitySlot {
                generation: 0,
                state: EntityState::Alive,
            });

            entity
        }
    }

    fn despawn(&mut self, entity: Entity) -> bool {
        let Some(slot) = self.slots.get_mut(entity.index as usize) else {
            return false;
        };
        let exist = slot.state == EntityState::Alive && slot.generation == entity.generation;
        if exist {
            slot.state = EntityState::Free;
            slot.generation = slot.generation.wrapping_add(1);
            self.free.push(entity.index);
        }
        exist
    }

    fn contains(&self, entity: Entity) -> bool {
        let Some(slot) = self.slots.get(entity.index as usize) else {
            return false;
        };

        slot.state == EntityState::Alive && slot.generation == entity.generation
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn spawn_entity() {
        let mut entities = Entities::new();

        let a = entities.spawn();
        let b = entities.spawn();

        assert_ne!(a, b);
        assert!(entities.contains(a));
        assert!(entities.contains(b));
    }

    #[test]
    fn despawn_entity() {
        let mut entities = Entities::new();

        let entity = entities.spawn();

        assert!(entities.despawn(entity));
        assert!(!entities.contains(entity));
    }

    #[test]
    fn reuse_entity_id() {
        let mut entities = Entities::new();

        let a = entities.spawn();
        let _b = entities.spawn();

        entities.despawn(a);

        let c = entities.spawn();

        assert_eq!(a.index, c.index);
        assert_ne!(a.generation, c.generation);

        assert_ne!(a, c);
        assert!(entities.contains(c));
    }

    #[test]
    fn stale_entity_is_invalid() {
        let mut entities = Entities::new();

        let old = entities.spawn();

        entities.despawn(old);

        let new = entities.spawn();

        assert_eq!(old.index, new.index);
        assert!(!entities.contains(old));
        assert!(entities.contains(new));
    }

    #[test]
    fn despawn_twice() {
        let mut entities = Entities::new();

        let entity = entities.spawn();

        assert!(entities.despawn(entity));
        assert!(!entities.despawn(entity));
    }

    #[test]
    fn despawn_stale_entity() {
        let mut entities = Entities::new();

        let old = entities.spawn();
        entities.despawn(old);

        let new = entities.spawn();

        assert!(!entities.despawn(old));
        assert!(entities.contains(new));
    }

    #[test]
    fn multiple_spawn_and_despawn() {
        let mut entities = Entities::new();

        let a = entities.spawn();
        let b = entities.spawn();
        let c = entities.spawn();

        entities.despawn(a);
        entities.despawn(c);

        let d = entities.spawn();
        let e = entities.spawn();

        assert!(entities.contains(b));
        assert!(entities.contains(d));
        assert!(entities.contains(e));

        assert!(!entities.contains(a));
        assert!(!entities.contains(c));
    }

    #[test]
    fn despawn_invalid_id() {
        let mut entities = Entities::new();

        let invalid = Entity {
            index: 999,
            generation: 0,
        };

        assert!(!entities.despawn(invalid));
        assert!(!entities.contains(invalid));
    }
}
