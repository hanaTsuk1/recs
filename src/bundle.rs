use crate::{component::ComponentRegistry, entity::Entity};

pub trait Bundle {
    fn insert(self, entity: Entity, registry: &mut ComponentRegistry);
}

impl Bundle for () {
    fn insert(self, _entity: Entity, _registry: &mut ComponentRegistry) {}
}

#[allow(non_snake_case)]
impl<A: 'static> Bundle for (A,) {
    fn insert(self, entity: Entity, registry: &mut ComponentRegistry) {
        let (A,) = self;
        registry.insert(entity, A);
    }
}

#[allow(non_snake_case)]
impl<A: 'static, B: 'static> Bundle for (A, B) {
    fn insert(self, entity: Entity, registry: &mut ComponentRegistry) {
        let (A, B) = self;
        registry.insert(entity, A);
        registry.insert(entity, B);
    }
}
