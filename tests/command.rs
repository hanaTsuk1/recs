use std::sync::{Arc, Mutex};

use recs::{
    Commands, Component, Entity, IntoSystemConfig, Query, Schedule, SingleThreadedExecutor, World,
};

#[derive(Component)]
struct Position(i32);

#[derive(Component)]
struct Marker;

static ERRORS: Mutex<Vec<String>> = Mutex::new(Vec::new());

fn count_positions(world: &mut World) -> usize {
    world.query::<&Position>().count()
}

#[test]
fn commands_are_applied_after_the_batch() {
    let mut schedule = Schedule::default();
    schedule.add_system(|mut commands: Commands| {
        commands.spawn(Position(1));
        commands.spawn(Position(2));
    });

    let mut world = World::new();
    schedule.run(&mut world);

    assert_eq!(count_positions(&mut world), 2);
}

#[test]
fn query_and_commands_share_one_system() {
    let mut world = World::new();
    world.spawn(Position(2));

    let mut schedule = Schedule::default();
    // 同时使用 Query 和 Commands 的闭包 system
    schedule.add_system(|(positions, mut commands): (Query<&Position>, Commands)| {
        if positions.filter(|position| position.0 == 2).count() == 1 {
            commands.spawn(Marker);
        }
    });

    schedule.run(&mut world);

    assert_eq!(world.query::<&Marker>().count(), 1);
}

#[test]
fn same_batch_systems_do_not_see_each_others_commands() {
    let seen = Arc::new(Mutex::new(None));
    let seen_in_reader = seen.clone();

    let mut schedule = Schedule::default();
    schedule.set_executor(SingleThreadedExecutor);
    schedule.add_system(|mut commands: Commands| {
        commands.spawn(Position(1));
    });
    schedule.add_system(move |positions: Query<&Position>| {
        *seen_in_reader.lock().unwrap() = Some(positions.count());
    });

    let mut world = World::new();
    schedule.run(&mut world);

    // 命令在当前批次结束后才统一 apply，同批的其它 system 看不到
    assert_eq!(*seen.lock().unwrap(), Some(0));
    assert_eq!(count_positions(&mut world), 1);
}

#[test]
fn later_batch_sees_commands_of_earlier_batch() {
    let mut schedule = Schedule::default();
    schedule.set_executor(SingleThreadedExecutor);

    let spawner = |mut commands: Commands| {
        commands.spawn(Position(7));
    };
    let reader = move |(positions, mut commands): (Query<&Position>, Commands)| {
        if positions.count() > 0 {
            commands.spawn(Marker);
        }
    };
    schedule.add_system(spawner);
    schedule.add_system(reader.after(spawner));

    let mut world = World::new();
    schedule.run(&mut world);

    assert_eq!(count_positions(&mut world), 1);
    assert_eq!(world.query::<&Marker>().count(), 1);
}

#[test]
fn despawn_removes_entity() {
    let mut world = World::new();
    let entity = world.spawn(Position(1));

    let mut schedule = Schedule::default();
    schedule.add_system(move |mut commands: Commands| commands.despawn(entity));
    schedule.run(&mut world);

    assert_eq!(count_positions(&mut world), 0);
}

#[test]
#[should_panic(expected = "failed to run")]
fn default_handler_panics_on_command_error() {
    let mut world = World::new();
    let dead: Entity = world.spawn(Position(1));
    world.despawn(dead).unwrap();

    let mut schedule = Schedule::default();
    schedule.add_system(move |mut commands: Commands| commands.despawn(dead));
    schedule.run(&mut world);
}

#[test]
fn custom_error_handler_receives_command_error() {
    ERRORS.lock().unwrap().clear();

    let mut world = World::new();
    let dead: Entity = world.spawn(Position(1));
    world.despawn(dead).unwrap();

    let mut schedule = Schedule::default();
    schedule.add_system(move |mut commands: Commands| commands.despawn(dead));
    schedule.set_error_handler(|error| ERRORS.lock().unwrap().push(format!("{error:?}")));
    schedule.run(&mut world);

    let errors = ERRORS.lock().unwrap();
    assert_eq!(errors.len(), 1);
    assert!(errors[0].contains("NoSuchEntity"));
}

#[test]
fn multithreaded_executor_applies_every_batch() {
    let mut schedule = Schedule::default();
    schedule.add_system(|mut commands: Commands| {
        for _ in 0..10 {
            commands.spawn(Position(1));
        }
    });
    schedule.add_system(|mut commands: Commands| {
        for _ in 0..10 {
            commands.spawn(Position(2));
        }
    });

    let mut world = World::new();
    schedule.run(&mut world);

    assert_eq!(count_positions(&mut world), 20);
}

#[test]
fn commands_in_one_batch_apply_in_registration_order() {
    let mut schedule = Schedule::default();
    schedule.set_executor(SingleThreadedExecutor);
    schedule.add_system(|mut commands: Commands| commands.spawn(Position(1)));
    schedule.add_system(|mut commands: Commands| commands.spawn(Position(2)));

    let mut world = World::new();
    schedule.run(&mut world);

    // 同一批的命令按 SystemId（注册顺序）依次 apply，插入顺序与之一致
    let positions: Vec<i32> = world
        .query::<&Position>()
        .map(|position| position.0)
        .collect();
    assert_eq!(positions, [1, 2]);
}
