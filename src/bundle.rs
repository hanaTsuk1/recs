use variadics_please::all_tuples;

use crate::{
    component::{Component, ComponentRegistry},
    entity::Entity,
};

pub trait Bundle {
    fn insert(self, entity: Entity, registry: &mut ComponentRegistry);
}

impl Bundle for () {
    fn insert(self, _entity: Entity, _registry: &mut ComponentRegistry) {}
}

macro_rules! impl_bundle {
    ($($T:ident),*) => {
        impl<$($T: Component),*> Bundle for ($($T,)*) {
            fn insert(self, entity: Entity, registry: &mut ComponentRegistry) {
                #[allow(non_snake_case)]
                let ($($T,)*) = self;
                $(
                    registry.insert(entity, $T);
                )*
            }
        }
    };
}

all_tuples!(impl_bundle, 1, 16, T);
