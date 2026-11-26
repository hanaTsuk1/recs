use variadics_please::{all_tuples, all_tuples_enumerated};

use crate::{
    Component, World,
    access::Access,
    entity::Entity,
    error::RecsError,
    sparse_set::{SparseSetRawParts, SparseSetRawPartsMut},
    system::SystemParam,
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

unsafe impl<T: QueryData> SystemParam for Query<'_, T> {
    type Param<'w, 's> = Query<'w, T>;

    type State = ();

    unsafe fn get_param<'w, 's>(
        world: &WorldCell<'w>,
        _state: &'s mut Self::State,
    ) -> Self::Param<'w, 's> {
        unsafe { Query::init(world) }
    }

    fn init_access(access: &mut Access) {
        T::init_access(access);
    }

    fn init_state() -> Self::State {}

    fn apply(_world: &mut World, _state: &mut Self::State) -> Vec<RecsError> {
        vec![]
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
        let storage = unsafe { world.raw_parts::<T>() }?;
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
    storage: SparseSetRawPartsMut<'w, T>,
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
