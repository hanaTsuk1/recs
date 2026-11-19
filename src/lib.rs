#![allow(dead_code)]

mod bundle;
mod component;
mod entity;
mod error;
mod query;
mod sparse_set;
mod world;

pub use component::Component;
pub use recs_macro::Component;
