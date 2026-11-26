use std::marker::PhantomData;

use variadics_please::{all_tuples, all_tuples_enumerated};

use crate::{
    World,
    access::{Access, validate_access_or_panic},
    error::RecsError,
    world::WorldCell,
};

pub trait System: Send + 'static {
    fn init_access(&self, access: &mut Access);

    fn run(&mut self, world: &WorldCell<'_>);

    fn apply_deferred(&mut self, _world: &mut World) -> Vec<RecsError> {
        vec![]
    }
}

/// # Safety
///
/// `init_access` 必须准确描述 `get_param` 返回值会进行的所有组件访问，所有可变访问都必须声明为写入
///
/// 访问集合通过冲突检查后，`get_param` 及其返回值仍必须遵守 Rust 的引用别名规则
pub unsafe trait SystemParam {
    type Param<'w>;

    type State: Send + 'static;

    /// # Safety
    ///
    /// 调用前，`init_access` 声明的访问集合必须已通过冲突检查
    unsafe fn get_param<'w>(world: &WorldCell<'w>, state: &'w mut Self::State) -> Self::Param<'w>;

    fn init_access(access: &mut Access);

    fn init_state() -> Self::State;

    fn apply(world: &mut World, state: &mut Self::State) -> Vec<RecsError>;
}

unsafe impl SystemParam for () {
    type Param<'w> = ();

    type State = ();

    unsafe fn get_param<'w>(
        _world: &WorldCell<'w>,
        _state: &'w mut Self::State,
    ) -> Self::Param<'w> {
    }

    fn init_access(_access: &mut Access) {}

    fn init_state() -> Self::State {}

    fn apply(_world: &mut World, _state: &mut Self::State) -> Vec<RecsError> {
        vec![]
    }
}

macro_rules! impl_system_param {
    ($(($n:tt, $T:ident)),*) => {
        unsafe impl<$($T: SystemParam),*> SystemParam for ($($T,)*) {
            type Param<'w> = ($($T::Param<'w>,)*);

            type State = ($($T::State,)*);

            unsafe fn get_param<'w>(
                world: &WorldCell<'w>,
                state: &'w mut Self::State,
            ) -> Self::Param<'w> {
                (
                    $(unsafe { $T::get_param(world, &mut state.$n) },)*
                )
            }

            fn init_access(access: &mut Access) {
                $(
                    $T::init_access(access);
                )*
            }

            fn init_state() -> Self::State {
                (
                    $($T::init_state(),)*
                )
            }

            fn apply(world: &mut World, state: &mut Self::State) -> Vec<RecsError> {
                let mut errors = Vec::new();

                $(
                    errors.extend($T::apply(world, &mut state.$n));
                )*

                errors
            }
        }
    }
}

all_tuples_enumerated!(impl_system_param, 1, 16, T);

pub struct FunctionSystem<T, Marker>
where
    T: SystemParamFunction<Marker>,
{
    func: T,
    state: Option<<T::Param as SystemParam>::State>,
    _marker: PhantomData<Marker>,
}

impl<T, Marker> FunctionSystem<T, Marker>
where
    T: SystemParamFunction<Marker>,
{
    fn initialize(&mut self) {
        self.state.get_or_insert_with(|| {
            validate_access_or_panic(T::Param::init_access);
            T::Param::init_state()
        });
    }
}

impl<T, Marker: Send + 'static> System for FunctionSystem<T, Marker>
where
    T: SystemParamFunction<Marker> + Send + 'static,
{
    fn init_access(&self, access: &mut Access) {
        T::Param::init_access(access);
    }

    fn run(&mut self, world: &WorldCell<'_>) {
        self.initialize();
        let state = self.state.as_mut().expect("failed to init state");
        // SAFETY: `SystemParam` 的`init_access()` 已保证通过冲突检查
        let param = unsafe { T::Param::get_param(world, state) };
        T::run(&mut self.func, param);
    }

    fn apply_deferred(&mut self, world: &mut World) -> Vec<RecsError> {
        if let Some(state) = &mut self.state {
            T::Param::apply(world, state)
        } else {
            vec![]
        }
    }
}

pub trait IntoSystem<Marker> {
    type System: System;

    fn into_system(self) -> Self::System;
}

macro_rules! impl_into_system {
    ($($T:ident),*) => {
        impl<T, $($T),*> IntoSystem<fn($($T,)*)> for T
        where
            $(
                $T: SystemParam + 'static,
            )*
            for<'a> &'a mut T: FnMut($($T),*),
            T: SystemParamFunction<fn($($T),*)> + Send + 'static,
        {
            type System = FunctionSystem<T, fn($($T),*)>;

            fn into_system(self) -> Self::System {
                FunctionSystem {
                    func: self,
                    state: None,
                    _marker: PhantomData,
                }
            }
        }
    }
}

all_tuples!(impl_into_system, 1, 16, T);

pub trait SystemParamFunction<Marker> {
    type Param: SystemParam;

    fn run<'w>(&mut self, param: <Self::Param as SystemParam>::Param<'w>);
}

macro_rules! impl_system_param_function {
    ($(($n:tt, $T:ident)),*) => {
        impl<T, $($T),*> SystemParamFunction<fn($($T,)*)> for T
        where
            $(
                $T: SystemParam,
            )*
            T: for<'w> FnMut($($T::Param<'w>,)*),
        {
            type Param = ($($T,)*);

            fn run<'w>(&mut self, param: <Self::Param as SystemParam>::Param<'w>) {
                self($(param.$n,)*);
            }
        }
    }
}

all_tuples_enumerated!(impl_system_param_function, 1, 16, T);

#[cfg(test)]
mod tests {
    use super::*;

    struct NoDeferred;

    impl System for NoDeferred {
        fn init_access(&self, _access: &mut Access) {}

        fn run(&mut self, _world: &WorldCell<'_>) {}
    }

    #[test]
    fn apply_deferred_defaults_to_no_errors() {
        let mut world = World::new();

        assert!(NoDeferred.apply_deferred(&mut world).is_empty());
    }
}
