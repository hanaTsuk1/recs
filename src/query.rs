use std::{
    any::{TypeId, type_name},
    collections::HashMap,
    marker::PhantomData,
};

use crate::{
    Component,
    entity::Entity,
    error::{RecsError, RecsResult},
    sparse_set::SparseSetRawParts,
    world::World,
};

pub struct Query<'w, T: QueryData> {
    fetch: Option<T::Fetch<'w>>,
    cursor: usize,
    _marker: PhantomData<&'w mut World>,
}

impl<'w, T: QueryData> Query<'w, T> {
    pub fn init(world: &'w mut World) -> Self {
        let world_ptr = world as *mut World;
        let access = T::access();
        if let Err(err) = validate_access(access) {
            panic!("query access conflict: {err:?}")
        }
        let fetch = unsafe { T::init(world_ptr) };
        Self {
            fetch,
            cursor: 0,
            _marker: PhantomData,
        }
    }
}

impl<'w, T: QueryData + 'w> Iterator for Query<'w, T> {
    type Item = T::Item<'w>;

    fn next(&mut self) -> Option<Self::Item> {
        let fetch = self.fetch.as_mut()?;

        while self.cursor < fetch.candidate_len() {
            let index = self.cursor;
            self.cursor += 1;

            let Some(entity) = (unsafe { fetch.candidate_at(index) }) else {
                continue;
            };

            if let Some(item) = fetch.fetch(entity) {
                return Some(item);
            }
        }

        None
    }
}

pub struct Access {
    kind: AccessKind,
    type_id: TypeId,
    type_name: &'static str,
}

#[derive(PartialEq)]
pub enum AccessKind {
    Read,
    Write,
}

fn validate_access(accesses: Vec<Access>) -> RecsResult<()> {
    let mut seen = HashMap::<TypeId, Access>::new();

    for access in accesses {
        if let Some(previous) = seen.get(&access.type_id) {
            if previous.kind != AccessKind::Read || access.kind != AccessKind::Read {
                return Err(RecsError::AccessConflict {
                    name: access.type_name,
                });
            }
        } else {
            seen.insert(access.type_id, access);
        }
    }

    Ok(())
}

pub trait QueryData {
    type Item<'w>;
    type Fetch<'w>: Fetch<'w, Item = Self::Item<'w>>;

    unsafe fn init<'w>(world: *mut World) -> Option<Self::Fetch<'w>>;
    fn access() -> Vec<Access>;
}

unsafe fn raw_parts<T: Component>(world: *mut World) -> Option<SparseSetRawParts<T>> {
    let world = unsafe { &mut *world };
    world.components.raw_parts_mut::<T>()
}

impl<T: Component> QueryData for &T {
    type Item<'w> = &'w T;
    type Fetch<'w> = ReadFetch<'w, T>;

    unsafe fn init<'w>(world: *mut World) -> Option<Self::Fetch<'w>> {
        Some(ReadFetch {
            storage: unsafe { raw_parts(world) }?,
            _marker: PhantomData,
        })
    }

    fn access() -> Vec<Access> {
        vec![Access {
            kind: AccessKind::Read,
            type_id: TypeId::of::<T>(),
            type_name: type_name::<T>(),
        }]
    }
}

impl<T: Component> QueryData for &mut T {
    type Item<'w> = &'w mut T;
    type Fetch<'w> = WriteFetch<'w, T>;

    unsafe fn init<'w>(world: *mut World) -> Option<Self::Fetch<'w>> {
        Some(WriteFetch {
            storage: unsafe { raw_parts(world) }?,
            _marker: PhantomData,
        })
    }

    fn access() -> Vec<Access> {
        vec![Access {
            kind: AccessKind::Write,
            type_id: TypeId::of::<T>(),
            type_name: type_name::<T>(),
        }]
    }
}

impl<A: QueryData, B: QueryData> QueryData for (A, B) {
    type Item<'w> = (A::Item<'w>, B::Item<'w>);
    type Fetch<'w> = (A::Fetch<'w>, B::Fetch<'w>);

    unsafe fn init<'w>(world: *mut World) -> Option<Self::Fetch<'w>> {
        let a = unsafe { A::init(world)? };
        let b = unsafe { B::init(world)? };
        Some((a, b))
    }

    fn access() -> Vec<Access> {
        let mut result = vec![];
        result.extend(A::access());
        result.extend(B::access());
        result
    }
}

pub trait Fetch<'w> {
    type Item;

    fn candidate_len(&self) -> usize;

    unsafe fn candidate_at(&self, index: usize) -> Option<Entity>;

    fn fetch(&mut self, entity: Entity) -> Option<Self::Item>;
}

pub struct ReadFetch<'w, T> {
    storage: SparseSetRawParts<T>,
    _marker: PhantomData<&'w T>,
}

pub struct WriteFetch<'w, T> {
    storage: SparseSetRawParts<T>,
    _marker: PhantomData<&'w mut T>,
}

impl<'w, T: 'w> Fetch<'w> for ReadFetch<'w, T> {
    type Item = &'w T;

    fn fetch(&mut self, entity: Entity) -> Option<Self::Item> {
        unsafe { self.storage.get(entity) }
    }

    fn candidate_len(&self) -> usize {
        self.storage.len()
    }

    unsafe fn candidate_at(&self, index: usize) -> Option<Entity> {
        unsafe { self.storage.entity_at(index) }
    }
}

impl<'w, T: 'w> Fetch<'w> for WriteFetch<'w, T> {
    type Item = &'w mut T;

    fn fetch(&mut self, entity: Entity) -> Option<Self::Item> {
        unsafe { self.storage.get_mut(entity) }
    }

    fn candidate_len(&self) -> usize {
        self.storage.len()
    }

    unsafe fn candidate_at(&self, index: usize) -> Option<Entity> {
        unsafe { self.storage.entity_at(index) }
    }
}

impl<'w, A: Fetch<'w>, B: Fetch<'w>> Fetch<'w> for (A, B) {
    type Item = (A::Item, B::Item);

    fn fetch(&mut self, entity: Entity) -> Option<Self::Item> {
        let a = A::fetch(&mut self.0, entity)?;
        let b = B::fetch(&mut self.1, entity)?;
        Some((a, b))
    }

    fn candidate_len(&self) -> usize {
        let a = A::candidate_len(&self.0);
        let b = B::candidate_len(&self.1);
        a.min(b)
    }

    unsafe fn candidate_at(&self, index: usize) -> Option<Entity> {
        if A::candidate_len(&self.0) <= B::candidate_len(&self.1) {
            unsafe { A::candidate_at(&self.0, index) }
        } else {
            unsafe { B::candidate_at(&self.1, index) }
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::{Component, world::World};

    #[derive(Component)]
    struct Position(i32);

    #[derive(Component)]
    struct Velocity(i32);

    #[derive(Component)]
    struct Health(i32);

    #[test]
    fn read_query_returns_all_matching_components() {
        let mut world = World::new();
        world.spawn(Position(10));
        world.spawn(Position(20));
        world.spawn(Velocity(30));

        let mut values = world
            .query::<&Position>()
            .map(|position| position.0)
            .collect::<Vec<_>>();
        values.sort_unstable();

        assert_eq!(values, [10, 20]);
    }

    #[test]
    fn write_query_mutates_components() {
        let mut world = World::new();
        let first = world.spawn(Position(10));
        let second = world.spawn(Position(20));

        for position in world.query::<&mut Position>() {
            position.0 += 1;
        }

        assert_eq!(world.get::<Position>(first).unwrap().unwrap().0, 11);
        assert_eq!(world.get::<Position>(second).unwrap().unwrap().0, 21);
    }

    #[test]
    fn tuple_read_query_joins_components_by_entity() {
        let mut world = World::new();
        world.spawn((Position(1), Velocity(10)));
        world.spawn(Position(2));
        world.spawn((Position(3), Velocity(30)));

        let mut rows = world
            .query::<(&Position, &Velocity)>()
            .map(|(position, velocity)| (position.0, velocity.0))
            .collect::<Vec<_>>();
        rows.sort_unstable();

        assert_eq!(rows, [(1, 10), (3, 30)]);
    }

    #[test]
    fn tuple_query_skips_nonmatching_entities_and_continues() {
        let mut world = World::new();
        world.spawn((Position(1), Velocity(10)));
        world.spawn(Velocity(20));
        world.spawn((Position(3), Velocity(30)));
        world.spawn(Position(4));
        world.spawn(Position(5));

        let mut rows = world
            .query::<(&Position, &Velocity)>()
            .map(|(position, velocity)| (position.0, velocity.0))
            .collect::<Vec<_>>();
        rows.sort_unstable();

        assert_eq!(rows, [(1, 10), (3, 30)]);
    }

    #[test]
    fn tuple_write_query_only_mutates_entities_with_both_components() {
        let mut world = World::new();
        let matched = world.spawn((Position(1), Velocity(10)));
        let position_only = world.spawn(Position(2));
        let velocity_only = world.spawn(Velocity(30));

        for (position, velocity) in world.query::<(&mut Position, &Velocity)>() {
            position.0 += velocity.0;
        }

        assert_eq!(world.get::<Position>(matched).unwrap().unwrap().0, 11);
        assert_eq!(world.get::<Position>(position_only).unwrap().unwrap().0, 2);
        assert_eq!(world.get::<Velocity>(velocity_only).unwrap().unwrap().0, 30);
    }

    #[test]
    fn query_for_missing_storage_is_empty() {
        let mut world = World::new();
        world.spawn(Position(1));

        assert_eq!(world.query::<&Health>().count(), 0);
        assert_eq!(world.query::<(&Position, &Health)>().count(), 0);
    }

    #[test]
    fn tuple_query_is_empty_when_storages_have_no_common_entity() {
        let mut world = World::new();
        world.spawn(Position(1));
        world.spawn(Velocity(10));

        assert_eq!(world.query::<(&Position, &Velocity)>().count(), 0);
    }

    #[test]
    fn repeated_read_access_to_same_component_is_allowed() {
        let mut world = World::new();
        world.spawn(Position(7));

        let values = world
            .query::<(&Position, &Position)>()
            .map(|(first, second)| (first.0, second.0))
            .collect::<Vec<_>>();

        assert_eq!(values, [(7, 7)]);
    }

    #[test]
    #[should_panic(expected = "query access conflict")]
    fn read_write_access_to_same_component_is_rejected() {
        let mut world = World::new();
        world.spawn(Position(7));

        let _query = world.query::<(&Position, &mut Position)>();
    }

    #[test]
    #[should_panic(expected = "query access conflict")]
    fn write_read_access_to_same_component_is_rejected() {
        let mut world = World::new();
        world.spawn(Position(7));

        let _query = world.query::<(&mut Position, &Position)>();
    }

    #[test]
    #[should_panic(expected = "query access conflict")]
    fn repeated_write_access_to_same_component_is_rejected() {
        let mut world = World::new();
        world.spawn(Position(7));

        let _query = world.query::<(&mut Position, &mut Position)>();
    }

    #[test]
    #[should_panic(expected = "query access conflict")]
    fn nested_tuple_access_conflict_is_rejected() {
        let mut world = World::new();
        world.spawn((Position(7), Velocity(3)));

        let _query = world.query::<((&mut Position, &Velocity), &Position)>();
    }
}
