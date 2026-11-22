use std::{
    any::{TypeId, type_name},
    collections::HashMap,
};

use crate::{
    Component,
    error::{RecsError, RecsResult},
};

#[derive(Default)]
pub struct Access {
    accesses: HashMap<TypeId, AccessKind>,
    conflict: Option<&'static str>,
}

impl Access {
    pub fn add_read<T: Component>(&mut self) {
        self.add::<T>(AccessKind::Read);
    }

    pub fn add_write<T: Component>(&mut self) {
        self.add::<T>(AccessKind::Write);
    }

    fn add<T: Component>(&mut self, kind: AccessKind) {
        let type_id = TypeId::of::<T>();
        let type_name = type_name::<T>();

        if let Some(previous) = self.accesses.get(&type_id) {
            if *previous != AccessKind::Read || kind != AccessKind::Read {
                self.conflict.get_or_insert(type_name);
            }
        } else {
            self.accesses.insert(type_id, kind);
        }
    }

    pub fn validate(&self) -> RecsResult<()> {
        match self.conflict {
            Some(name) => Err(RecsError::AccessConflict { name }),
            None => Ok(()),
        }
    }
}

pub(crate) fn validate_access_or_panic(init_access: impl FnOnce(&mut Access)) {
    let mut access = Access::default();
    init_access(&mut access);
    if let Err(err) = access.validate() {
        panic!("query access conflict: {err:?}")
    }
}

#[derive(PartialEq)]
pub enum AccessKind {
    Read,
    Write,
}
