use std::time::Duration;

use bevy_ecs::component::Component as BevyComponent;
use criterion::{BenchmarkGroup, measurement::WallTime};
use recs::Component as RecsComponent;

pub const ITER_COUNTS: [usize; 3] = [10_000, 100_000, 1_000_000];
pub const SMALL_COUNTS: [usize; 2] = [10_000, 100_000];
pub const STRUCTURAL_COUNTS: [usize; 3] = [1_000, 10_000, 100_000];

pub trait Scalar {
    fn value(&self) -> f64;
}

pub trait PositionScalar: Scalar {
    fn add_velocity(&mut self, velocity: f64);
}

macro_rules! define_components {
    ($recs:ident, $table:ident, $sparse:ident) => {
        #[derive(RecsComponent, Clone, Copy)]
        pub struct $recs(pub f64);

        #[derive(BevyComponent, Clone, Copy)]
        pub struct $table(pub f64);

        #[derive(BevyComponent, Clone, Copy)]
        #[component(storage = "SparseSet")]
        pub struct $sparse(pub f64);

        impl Scalar for $recs {
            fn value(&self) -> f64 {
                self.0
            }
        }

        impl Scalar for $table {
            fn value(&self) -> f64 {
                self.0
            }
        }

        impl Scalar for $sparse {
            fn value(&self) -> f64 {
                self.0
            }
        }

        impl PositionScalar for $recs {
            fn add_velocity(&mut self, velocity: f64) {
                self.0 += velocity;
            }
        }

        impl PositionScalar for $table {
            fn add_velocity(&mut self, velocity: f64) {
                self.0 += velocity;
            }
        }

        impl PositionScalar for $sparse {
            fn add_velocity(&mut self, velocity: f64) {
                self.0 += velocity;
            }
        }
    };
}

define_components!(RPosition, TPosition, SPosition);
define_components!(RVelocity, TVelocity, SVelocity);
define_components!(RToggle, TToggle, SToggle);

#[derive(RecsComponent)]
pub struct RTag0;
#[derive(RecsComponent)]
pub struct RTag1;
#[derive(RecsComponent)]
pub struct RTag2;
#[derive(RecsComponent)]
pub struct RTag3;

#[derive(BevyComponent)]
pub struct TTag0;
#[derive(BevyComponent)]
pub struct TTag1;
#[derive(BevyComponent)]
pub struct TTag2;
#[derive(BevyComponent)]
pub struct TTag3;

#[derive(BevyComponent)]
#[component(storage = "SparseSet")]
pub struct STag0;
#[derive(BevyComponent)]
#[component(storage = "SparseSet")]
pub struct STag1;
#[derive(BevyComponent)]
#[component(storage = "SparseSet")]
pub struct STag2;
#[derive(BevyComponent)]
#[component(storage = "SparseSet")]
pub struct STag3;

pub fn configure_group(group: &mut BenchmarkGroup<'_, WallTime>) {
    group.sample_size(20);
    group.warm_up_time(Duration::from_millis(500));
    group.measurement_time(Duration::from_secs(4));
}
