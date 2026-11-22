#![allow(dead_code)]

mod access;
mod bundle;
mod component;
mod entity;
mod error;
mod query;
mod sparse_set;
mod system;
mod world;

pub use bundle::Bundle;
pub use component::{Component, ComponentRegistry};
pub use entity::Entity;
pub use query::Query;
pub use world::World;

pub use recs_macro::Component;
