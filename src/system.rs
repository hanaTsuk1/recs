use std::marker::PhantomData;

use variadics_please::all_tuples;

use crate::{
    access::{Access, validate_access_or_panic},
    world::WorldCell,
};

pub trait System: 'static {
    fn init_access(&self, access: &mut Access);

    fn run(&mut self, world: &WorldCell<'_>);
}

/// # Safety
///
/// `init_access` 必须准确描述 `get_param` 返回值会进行的所有组件访问，所有可变访问都必须声明为写入
///
/// 访问集合通过冲突检查后，`get_param` 及其返回值仍必须遵守 Rust 的引用别名规则
pub unsafe trait SystemParam {
    type Param<'w>;
    /// # Safety
    ///
    /// 调用前，`init_access` 声明的访问集合必须已通过冲突检查
    unsafe fn get_param<'w>(world: &WorldCell<'w>) -> Self::Param<'w>;

    fn init_access(access: &mut Access);
}

unsafe impl SystemParam for () {
    type Param<'w> = ();

    unsafe fn get_param<'w>(_world: &WorldCell<'w>) -> Self::Param<'w> {}

    fn init_access(_access: &mut Access) {}
}

macro_rules! impl_system_param {
    ($($T:ident),*) => {
        unsafe impl<$($T: SystemParam),*> SystemParam for ($($T,)*) {
            type Param<'w> = ($($T::Param<'w>,)*);

            unsafe fn get_param<'w>(world: &WorldCell<'w>) -> Self::Param<'w> {
                (
                    $(unsafe { $T::get_param(world) },)*
                )
            }

            fn init_access(access: &mut Access) {
                $(
                    $T::init_access(access);
                )*
            }
        }
    }
}

all_tuples!(impl_system_param, 1, 16, T);

pub struct FunctionSystem<T, Marker> {
    func: T,
    _marker: PhantomData<Marker>,
}

impl<T, Marker: 'static> System for FunctionSystem<T, Marker>
where
    T: SystemParamFunction<Marker> + 'static,
{
    fn init_access(&self, access: &mut Access) {
        <<T as SystemParamFunction<Marker>>::Param as SystemParam>::init_access(access);
    }

    fn run(&mut self, world: &WorldCell<'_>) {
        validate_access_or_panic(|access| self.init_access(access));
        // SAFETY: `SystemParam` 的`init_access()` 已保证通过冲突检查
        let param =
            unsafe { <<T as SystemParamFunction<Marker>>::Param as SystemParam>::get_param(world) };
        <T as SystemParamFunction<Marker>>::run(&mut self.func, param);
    }
}

pub trait IntoSystem<Marker> {
    type System: System;

    fn into_system(self) -> Self::System;
}

impl<T, P: SystemParam + 'static> IntoSystem<fn(P)> for T
where
    for<'a> &'a mut T: FnMut(P),
    T: SystemParamFunction<fn(P)> + 'static,
{
    type System = FunctionSystem<T, fn(P)>;

    fn into_system(self) -> Self::System {
        FunctionSystem {
            func: self,
            _marker: PhantomData,
        }
    }
}

pub trait SystemParamFunction<Marker> {
    type Param: SystemParam;

    fn run<'w>(&mut self, param: <Self::Param as SystemParam>::Param<'w>);
}

impl<T, P> SystemParamFunction<fn(P)> for T
where
    P: SystemParam,
    T: for<'w> FnMut(P::Param<'w>),
{
    type Param = P;

    fn run<'w>(&mut self, param: <Self::Param as SystemParam>::Param<'w>) {
        self(param);
    }
}
