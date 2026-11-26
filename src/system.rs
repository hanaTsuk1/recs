use std::marker::PhantomData;

use variadics_please::all_tuples_enumerated;

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
    /// `'w` 是 world 的借用，`'s` 是参数自身状态的借用。
    ///
    /// 分成两个生命周期是为了让闭包 system 能同时使用 `Query` 和 `Commands`：
    /// `Query` 只依赖 `'w`，`Commands` 只依赖 `'s`，元组里各位置的
    /// 生命周期相互独立，闭包的生命周期推断才能匹配。
    type Param<'w, 's>;

    type State: Send + 'static;

    /// # Safety
    ///
    /// 调用前，`init_access` 声明的访问集合必须已通过冲突检查
    unsafe fn get_param<'w, 's>(
        world: &WorldCell<'w>,
        state: &'s mut Self::State,
    ) -> Self::Param<'w, 's>;

    fn init_access(access: &mut Access);

    fn init_state() -> Self::State;

    fn apply(world: &mut World, state: &mut Self::State) -> Vec<RecsError>;
}

unsafe impl SystemParam for () {
    type Param<'w, 's> = ();

    type State = ();

    unsafe fn get_param<'w, 's>(
        _world: &WorldCell<'w>,
        _state: &'s mut Self::State,
    ) -> Self::Param<'w, 's> {
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
            type Param<'w, 's> = ($($T::Param<'w, 's>,)*);

            type State = ($($T::State,)*);

            unsafe fn get_param<'w, 's>(
                world: &WorldCell<'w>,
                state: &'s mut Self::State,
            ) -> Self::Param<'w, 's> {
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

impl<T, P: SystemParam + 'static> IntoSystem<fn(P)> for T
where
    for<'a> &'a mut T: FnMut(P),
    T: SystemParamFunction<fn(P)> + Send + 'static,
{
    type System = FunctionSystem<T, fn(P)>;

    fn into_system(self) -> Self::System {
        FunctionSystem {
            func: self,
            state: None,
            _marker: PhantomData,
        }
    }
}

pub trait SystemParamFunction<Marker> {
    type Param: SystemParam;

    fn run<'w, 's>(&mut self, param: <Self::Param as SystemParam>::Param<'w, 's>);
}

impl<T, P> SystemParamFunction<fn(P)> for T
where
    P: SystemParam,
    T: for<'w, 's> FnMut(P::Param<'w, 's>),
{
    type Param = P;

    fn run<'w, 's>(&mut self, param: <Self::Param as SystemParam>::Param<'w, 's>) {
        self(param);
    }
}

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
