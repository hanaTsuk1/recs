use recs::{IntoSystemConfig, Schedule, World};
use std::sync::Mutex;

static EXECUTION_ORDER: Mutex<Vec<u8>> = Mutex::new(Vec::new());

fn system_one(_: ()) {
    EXECUTION_ORDER.lock().unwrap().push(1);
}

fn system_two(_: ()) {
    EXECUTION_ORDER.lock().unwrap().push(2);
}

fn system_three(_: ()) {
    EXECUTION_ORDER.lock().unwrap().push(3);
}

#[test]
fn run_executes_systems_in_configured_order() {
    EXECUTION_ORDER.lock().unwrap().clear();
    let mut schedule = Schedule::default();
    schedule.add_system(system_two);
    schedule.add_system(system_one.before(system_two));
    schedule.add_system(system_three.after(system_two).after(system_one));
    let mut world = World::new();

    schedule.run(&mut world);

    assert_eq!(*EXECUTION_ORDER.lock().unwrap(), [1, 2, 3]);
}

#[test]
#[should_panic(expected = "failed to build schedule plan")]
fn run_panics_for_an_unregistered_dependency_system() {
    let mut schedule = Schedule::default();
    schedule.add_system(system_one.before(system_two));

    schedule.run(&mut World::new());
}

#[test]
#[should_panic(expected = "failed to build schedule plan")]
fn run_panics_for_circular_dependencies() {
    let mut schedule = Schedule::default();
    schedule.add_system(system_one.before(system_two));
    schedule.add_system(system_two.before(system_one));

    schedule.run(&mut World::new());
}
