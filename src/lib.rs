#![allow(dead_code)]

mod bundle;
mod component;
mod entity;
mod error;
mod query;
mod sparse_set;
mod world;

pub use bundle::Bundle;
pub use component::{Component, ComponentRegistry};
pub use entity::Entity;
pub use world::World;

pub use recs_macro::Component;
