use variadics_please::all_tuples;

use crate::{
    access::{Access, validate_access_or_panic},
    world::WorldCell,
};

pub trait System<'w, P: SystemParam<'w>> {
    fn run(&mut self, world: &WorldCell<'w>);
}

impl<'w, T, P> System<'w, P> for T
where
    T: FnMut(P),
    P: SystemParam<'w>,
{
    fn run(&mut self, world: &WorldCell<'w>) {
        validate_access_or_panic(P::init_access);
        // SAFETY: `SystemParam` 的`init_access()` 已保证通过冲突检查
        let param = unsafe { P::get_param(world) };
        self(param);
    }
}

/// # Safety
///
/// `init_access` 必须准确描述 `get_param` 返回值会进行的所有组件访问，所有可变访问都必须声明为写入
///
/// 访问集合通过冲突检查后，`get_param` 及其返回值仍必须遵守 Rust 的引用别名规则
pub unsafe trait SystemParam<'w> {
    /// # Safety
    ///
    /// 调用前，`init_access` 声明的访问集合必须已通过冲突检查
    unsafe fn get_param(world: &WorldCell<'w>) -> Self;

    fn init_access(access: &mut Access);
}

unsafe impl<'w> SystemParam<'w> for () {
    unsafe fn get_param(_world: &WorldCell<'w>) -> Self {}

    fn init_access(_access: &mut Access) {}
}

macro_rules! impl_system_param {
    ($($T:ident),*) => {
        unsafe impl<'w, $($T: SystemParam<'w>),*> SystemParam<'w> for ($($T,)*) {
            unsafe fn get_param(world: &WorldCell<'w>) -> Self {
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

all_tuples!(impl_system_param, 2, 16, T);
