use std::{
    any::TypeId,
    collections::HashMap,
    ops::{Deref, DerefMut},
};

use crate::{Component, Entity, access::Access, system::SystemParam, world::WorldCell};

pub trait Resource: Component {}

#[derive(Default)]
pub struct ResourceEntities(HashMap<TypeId, Entity>);

impl ResourceEntities {
    pub fn insert<R: Resource>(&mut self, entity: Entity) {
        let type_id = TypeId::of::<R>();
        self.0.insert(type_id, entity);
    }

    pub fn get<R: Resource>(&self) -> Option<Entity> {
        let type_id = TypeId::of::<R>();
        self.0.get(&type_id).copied()
    }

    pub fn remove<R: Resource>(&mut self) -> Option<Entity> {
        let type_id = TypeId::of::<R>();
        self.0.remove(&type_id)
    }
}

pub struct Res<'w, T: Resource> {
    value: &'w T,
}

pub struct ResMut<'w, T: Resource> {
    value: &'w mut T,
}

impl<'w, T: Resource> Deref for Res<'w, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        self.value
    }
}

impl<'w, T: Resource> Deref for ResMut<'w, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        self.value
    }
}

impl<'w, T: Resource> DerefMut for ResMut<'w, T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.value
    }
}

unsafe impl<T: Resource> SystemParam for Res<'_, T> {
    type Param<'w> = Res<'w, T>;

    type State = ();

    unsafe fn get_param<'w>(world: &WorldCell<'w>, _state: &'w mut Self::State) -> Self::Param<'w> {
        let value = unsafe { world.get_resource::<T>().expect("resource does not exist") };
        Res { value }
    }

    fn init_access(access: &mut Access) {
        access.add_read::<T>();
    }

    fn init_state() -> Self::State {}
}

unsafe impl<T: Resource> SystemParam for ResMut<'_, T> {
    type Param<'w> = ResMut<'w, T>;

    type State = ();

    unsafe fn get_param<'w>(world: &WorldCell<'w>, _state: &'w mut Self::State) -> Self::Param<'w> {
        let value = unsafe {
            world
                .get_resource_mut::<T>()
                .expect("resource does not exist")
        };
        ResMut { value }
    }

    fn init_access(access: &mut Access) {
        access.add_write::<T>();
    }

    fn init_state() -> Self::State {}
}
