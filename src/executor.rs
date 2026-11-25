use crate::{World, schedule::SystemPlan, world::WorldCell};

pub trait SystemExecutor {
    fn run(&mut self, plan: &mut SystemPlan<'_>, world: &mut World);
}

pub struct SingleThreadedExecutor;

pub struct MultiThreadedExecutor;

impl SystemExecutor for SingleThreadedExecutor {
    fn run(&mut self, plan: &mut SystemPlan<'_>, world: &mut World) {
        let cell = WorldCell::new(world);
        for index in 0..plan.batch_count() {
            for system in plan.batch_systems(index) {
                system.run(&cell);
            }
        }
    }
}

impl SystemExecutor for MultiThreadedExecutor {
    fn run(&mut self, plan: &mut SystemPlan<'_>, world: &mut World) {
        let cell = WorldCell::new(world);
        for index in 0..plan.batch_count() {
            std::thread::scope(|scope| {
                for system in plan.batch_systems(index) {
                    scope.spawn(|| system.run(&cell));
                }
            });
        }
    }
}
