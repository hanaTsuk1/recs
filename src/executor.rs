use crate::{World, error::ErrorHandler, schedule::SystemPlan, world::WorldCell};

pub trait SystemExecutor {
    fn run(&mut self, plan: &mut SystemPlan<'_>, world: &mut World, error_handler: ErrorHandler);
}

pub struct SingleThreadedExecutor;

pub struct MultiThreadedExecutor;

impl SystemExecutor for SingleThreadedExecutor {
    fn run(&mut self, plan: &mut SystemPlan<'_>, world: &mut World, error_handler: ErrorHandler) {
        let mut errors = Vec::new();
        for index in 0..plan.batch_count() {
            let cell = WorldCell::new(world);
            let mut systems = plan.batch_systems(index);
            for system in &mut systems {
                system.run(&cell);
            }
            for system in systems {
                errors.extend(system.apply_deferred(world));
            }
        }
        for error in errors {
            error_handler(error);
        }
    }
}

impl SystemExecutor for MultiThreadedExecutor {
    fn run(&mut self, plan: &mut SystemPlan<'_>, world: &mut World, error_handler: ErrorHandler) {
        let mut errors = Vec::new();
        for index in 0..plan.batch_count() {
            let cell = WorldCell::new(world);
            let mut systems = plan.batch_systems(index);
            std::thread::scope(|scope| {
                for system in &mut systems {
                    scope.spawn(|| system.run(&cell));
                }
            });
            for system in systems {
                errors.extend(system.apply_deferred(world));
            }
        }
        for error in errors {
            error_handler(error);
        }
    }
}
