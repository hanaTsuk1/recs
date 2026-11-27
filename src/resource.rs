use std::{any::TypeId, collections::HashMap};

use crate::{Component, Entity};

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
