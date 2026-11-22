use variadics_please::{all_tuples, all_tuples_enumerated};

use crate::{
    Component, access::Access, entity::Entity, sparse_set::SparseSetRawParts, system::SystemParam,
    world::WorldCell,
};

pub struct Query<'w, T: QueryData> {
    fetch: Option<T::Fetch<'w>>,
    cursor: usize,
}

impl<'w, T: QueryData> Query<'w, T> {
    /// # Safety
    ///
    /// 调用前，`init_access` 声明的访问集合必须已通过冲突检查
    pub unsafe fn init(world: &WorldCell<'w>) -> Self {
        let fetch = unsafe { T::init_fetch(world) };
        Self { fetch, cursor: 0 }
    }
}

impl<'w, T: QueryData> Iterator for Query<'w, T> {
    type Item = T::Item<'w>;

    fn next(&mut self) -> Option<Self::Item> {
        let fetch = self.fetch.as_mut()?;

        while self.cursor < fetch.candidate_len() {
            let index = self.cursor;
            self.cursor += 1;

            let Some(entity) = fetch.candidate_at(index) else {
                continue;
            };

            // SAFETY: entity不会冲突
            if let Some(item) = unsafe { fetch.fetch(entity) } {
                return Some(item);
            }
        }

        None
    }
}

unsafe impl<'w, T: QueryData> SystemParam<'w> for Query<'w, T> {
    unsafe fn get_param(world: &WorldCell<'w>) -> Self {
        unsafe { Query::init(world) }
    }

    fn init_access(access: &mut Access) {
        T::init_access(access);
    }
}

/// # Safety
///
/// 实现者必须确保`access()`准确列出访问类型
///
/// 任何可变访问都必须标记为[`AccessKind::Write`]
///
/// 依赖此信息在`init`前拒绝冲突访问
pub unsafe trait QueryData {
    type Item<'w>;
    type Fetch<'w>: Fetch<'w, Item = Self::Item<'w>>;

    /// # Safety
    ///
    /// 调用方需独占world，完成fetch的初始化
    unsafe fn init_fetch<'w>(world: &WorldCell<'w>) -> Option<Self::Fetch<'w>>;

    fn init_access(access: &mut Access);
}

unsafe impl<T: Component> QueryData for &T {
    type Item<'w> = &'w T;
    type Fetch<'w> = ReadFetch<'w, T>;

    unsafe fn init_fetch<'w>(world: &WorldCell<'w>) -> Option<Self::Fetch<'w>> {
        let storage = unsafe { world.raw_parts_mut::<T>() }?;
        Some(ReadFetch { storage })
    }

    fn init_access(access: &mut Access) {
        access.add_read::<T>();
    }
}

unsafe impl<T: Component> QueryData for &mut T {
    type Item<'w> = &'w mut T;
    type Fetch<'w> = WriteFetch<'w, T>;

    unsafe fn init_fetch<'w>(world: &WorldCell<'w>) -> Option<Self::Fetch<'w>> {
        let storage = unsafe { world.raw_parts_mut::<T>() }?;
        Some(WriteFetch { storage })
    }

    fn init_access(access: &mut Access) {
        access.add_write::<T>();
    }
}

macro_rules! impl_query_data {
    ($($T:ident),*) => {
        unsafe impl<$($T: QueryData),*> QueryData for ($($T,)*) {
            type Item<'w> = ($($T::Item<'w>),*);
            type Fetch<'w> = ($($T::Fetch<'w>),*);

            unsafe fn init_fetch<'w>(world: &WorldCell<'w>) -> Option<Self::Fetch<'w>> {
                Some((
                    $(
                        unsafe { $T::init_fetch(world)? },
                    )*
                ))
            }

            fn init_access(access: &mut Access) {
                $(
                    $T::init_access(access);
                )*
            }
        }
    };
}

all_tuples!(impl_query_data, 2, 16, T);

/// # Safety
///
/// `fetch`必须能安全处理传入的任意entity，包括不在此fetch自身存储中的entity
pub unsafe trait Fetch<'w> {
    type Item;

    fn candidate_len(&self) -> usize;

    fn candidate_at(&self, index: usize) -> Option<Entity>;

    /// # Safety
    ///
    /// 调用方必须确保本次返回的项（如果有）不会与先前调用所返回且仍存活的项违反别名规则
    unsafe fn fetch(&mut self, entity: Entity) -> Option<Self::Item>;
}

pub struct ReadFetch<'w, T> {
    storage: SparseSetRawParts<'w, T>,
}

pub struct WriteFetch<'w, T> {
    storage: SparseSetRawParts<'w, T>,
}

unsafe impl<'w, T: 'w> Fetch<'w> for ReadFetch<'w, T> {
    type Item = &'w T;

    unsafe fn fetch(&mut self, entity: Entity) -> Option<Self::Item> {
        self.storage.get(entity)
    }

    fn candidate_len(&self) -> usize {
        self.storage.len()
    }

    fn candidate_at(&self, index: usize) -> Option<Entity> {
        self.storage.entity_at(index)
    }
}

unsafe impl<'w, T: 'w> Fetch<'w> for WriteFetch<'w, T> {
    type Item = &'w mut T;

    unsafe fn fetch(&mut self, entity: Entity) -> Option<Self::Item> {
        unsafe { self.storage.get_mut(entity) }
    }

    fn candidate_len(&self) -> usize {
        self.storage.len()
    }

    fn candidate_at(&self, index: usize) -> Option<Entity> {
        self.storage.entity_at(index)
    }
}

trait TupleFetchMeta<'w> {
    fn candidate_source(&self) -> (usize, usize);
}

macro_rules! impl_fetch {
    ($(($n:tt, $T:ident)),*) => {
        impl<'w, $($T: Fetch<'w>),*> TupleFetchMeta<'w> for ($($T,)*) {
            fn candidate_source(&self) -> (usize, usize) {
                [
                    $(
                        ($n, self.$n.candidate_len()),
                    )*
                ]
                .into_iter()
                .min_by_key(|(_, len)| *len)
                .unwrap_or((0, 0))
            }
        }

        unsafe impl<'w, $($T: Fetch<'w>),*> Fetch<'w> for ($($T,)*) {
            type Item = ($($T::Item,)*);

            unsafe fn fetch(&mut self, entity: Entity) -> Option<Self::Item> {
                Some((
                    $(
                        unsafe { self.$n.fetch(entity)? },
                    )*
                ))
            }

            fn candidate_len(&self) -> usize {
                self.candidate_source().1
            }

            fn candidate_at(&self, index: usize) -> Option<Entity> {
                let (candidate, _) = self.candidate_source();
                match candidate {
                    $(
                        $n => {
                            self.$n.candidate_at(index)
                        },
                    )*
                    _ => None,
                }
            }
        }
    };
}

all_tuples_enumerated!(impl_fetch, 2, 16, T);

#[cfg(test)]
mod tests {
    use crate::{Component, world::World};

    #[derive(Component)]
    struct Position(i32);

    #[derive(Component)]
    struct Velocity(i32);

    #[derive(Component)]
    struct Health(i32);

    #[derive(Component)]
    struct Armor(i32);

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
    fn three_component_query_joins_by_entity_and_skips_incomplete_rows() {
        let mut world = World::new();
        world.spawn((Position(1), Velocity(10), Health(100)));
        world.spawn((Position(2), Velocity(20)));
        world.spawn((Velocity(30), Health(300)));
        world.spawn((Position(4), Health(400)));
        world.spawn((Position(5), Velocity(50), Health(500)));

        let mut rows = world
            .query::<(&Position, &Velocity, &Health)>()
            .map(|(position, velocity, health)| (position.0, velocity.0, health.0))
            .collect::<Vec<_>>();
        rows.sort_unstable();

        assert_eq!(rows, [(1, 10, 100), (5, 50, 500)]);
    }

    #[test]
    fn four_component_query_joins_and_mutates_matching_entities() {
        let mut world = World::new();
        let matched = world.spawn((Position(1), Velocity(10), Health(100), Armor(1000)));
        let missing_armor = world.spawn((Position(2), Velocity(20), Health(200)));
        let missing_health = world.spawn((Position(3), Velocity(30), Armor(3000)));

        for (position, velocity, health, armor) in
            world.query::<(&mut Position, &Velocity, &Health, &Armor)>()
        {
            position.0 += velocity.0 + health.0 + armor.0;
        }

        assert_eq!(world.get::<Position>(matched).unwrap().unwrap().0, 1111);
        assert_eq!(world.get::<Position>(missing_armor).unwrap().unwrap().0, 2);
        assert_eq!(world.get::<Position>(missing_health).unwrap().unwrap().0, 3);
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
