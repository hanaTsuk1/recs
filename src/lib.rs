#![allow(dead_code)]

mod access;
mod bundle;
mod command;
mod component;
mod entity;
mod error;
mod executor;
mod query;
mod resource;
mod schedule;
mod sparse_set;
mod system;
mod world;

pub use bundle::Bundle;
pub use command::{Command, Commands};
pub use component::{Component, ComponentRegistry};
pub use entity::Entity;
pub use error::{ErrorHandler, RecsError, RecsResult};
pub use executor::{MultiThreadedExecutor, SingleThreadedExecutor};
pub use query::Query;
pub use resource::{Res, ResMut, Resource};
pub use schedule::{IntoSystemConfig, Schedule};
pub use world::World;

pub use recs_macro::{Component, Resource};
