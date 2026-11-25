use crate::{Schedule, World, world::WorldCell};

pub trait SystemExecutor {
    fn run(&mut self, schedule: &mut Schedule, world: &mut World);
}

pub struct SingleThreadedExecutor;

pub struct MultiThreadedExecutor;

impl SystemExecutor for SingleThreadedExecutor {
    fn run(&mut self, schedule: &mut Schedule, world: &mut World) {
        let cell = WorldCell::new(world);
        let batches = schedule.build_plan();
        for batch in batches {
            let systems = schedule.get_systems(&batch);
            for system in systems {
                system.run(&cell);
            }
        }
    }
}

impl SystemExecutor for MultiThreadedExecutor {
    fn run(&mut self, schedule: &mut Schedule, world: &mut World) {
        let cell = WorldCell::new(world);
        let batches = schedule.build_plan();
        for batch in batches {
            let systems = schedule.get_systems(&batch);
            std::thread::scope(|scope| {
                for system in systems {
                    scope.spawn(|| {
                        system.run(&cell);
                    });
                }
            });
        }
    }
}
