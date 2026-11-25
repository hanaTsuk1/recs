use std::{
    sync::{
        Arc, Mutex,
        atomic::{AtomicUsize, Ordering},
        mpsc,
    },
    thread,
    time::Duration,
};

use recs::{
    Component, IntoSystemConfig, MultiThreadedExecutor, Query, Schedule, SingleThreadedExecutor,
    World,
};

static EXECUTION_ORDER: Mutex<Vec<u8>> = Mutex::new(Vec::new());

#[derive(Component)]
struct Position(i32);

#[derive(Component)]
struct Velocity(i32);

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
fn run_executes_every_system_on_each_call() {
    let executions = Arc::new(AtomicUsize::new(0));
    let counter = executions.clone();

    let mut schedule = Schedule::default();
    schedule.set_executor(SingleThreadedExecutor);
    schedule.add_system(move |_: ()| {
        counter.fetch_add(1, Ordering::SeqCst);
    });

    let mut world = World::new();
    schedule.run(&mut world);
    schedule.run(&mut world);
    schedule.run(&mut world);

    assert_eq!(executions.load(Ordering::SeqCst), 3);
}

#[test]
fn adding_a_system_after_a_run_rebuilds_the_plan() {
    let log = Arc::new(Mutex::new(Vec::new()));

    let first_log = log.clone();
    let first = move |_: ()| first_log.lock().unwrap().push("first");
    let second_log = log.clone();
    let second = move |_: ()| second_log.lock().unwrap().push("second");

    let mut schedule = Schedule::default();
    schedule.set_executor(SingleThreadedExecutor);
    schedule.add_system(first.clone());

    let mut world = World::new();
    schedule.run(&mut world);
    assert_eq!(*log.lock().unwrap(), ["first"]);

    // 计划已缓存，若不把 schedule 标记为 dirty，新 system 永远不会被执行
    schedule.add_system(second.after(first.clone()));
    schedule.run(&mut world);

    assert_eq!(*log.lock().unwrap(), ["first", "first", "second"]);
}

#[test]
fn running_an_empty_schedule_does_nothing() {
    let mut schedule = Schedule::default();
    let mut world = World::new();

    schedule.run(&mut world);
    schedule.run(&mut world);
}

#[test]
fn systems_observe_changes_from_earlier_batches() {
    let mut world = World::new();
    let slow = world.spawn((Position(0), Velocity(2)));
    let fast = world.spawn((Position(10), Velocity(3)));

    let total = Arc::new(Mutex::new(None));
    let observed_total = total.clone();

    let integrate = |positions: Query<(&mut Position, &Velocity)>| {
        for (position, velocity) in positions {
            position.0 += velocity.0;
        }
    };
    let record = move |positions: Query<&Position>| {
        *observed_total.lock().unwrap() = Some(positions.map(|position| position.0).sum::<i32>());
    };

    let mut schedule = Schedule::default();
    schedule.add_system(integrate);
    // 读写同一组件本就无法同批执行，这里再显式约束先后以保证结果确定
    schedule.add_system(record.after(integrate));

    schedule.run(&mut world);

    assert_eq!(world.get::<Position>(slow).unwrap().unwrap().0, 2);
    assert_eq!(world.get::<Position>(fast).unwrap().unwrap().0, 13);
    assert_eq!(*total.lock().unwrap(), Some(15));
}

#[test]
fn independent_systems_in_one_batch_apply_all_world_changes() {
    let mut world = World::new();
    let entity = world.spawn((Position(0), Velocity(0)));

    let mut schedule = Schedule::default();
    schedule.add_system(|positions: Query<&mut Position>| {
        for position in positions {
            position.0 += 1;
        }
    });
    schedule.add_system(|velocities: Query<&mut Velocity>| {
        for velocity in velocities {
            velocity.0 += 10;
        }
    });

    schedule.run(&mut world);

    assert_eq!(world.get::<Position>(entity).unwrap().unwrap().0, 1);
    assert_eq!(world.get::<Velocity>(entity).unwrap().unwrap().0, 10);
}

/// 往 `schedule` 里加 `system_count` 个互不冲突的 system，每个 system 都在启动后
/// 阻塞到协调线程放行，最后返回真正同时启动的 system 数量。
///
/// - 多线程执行器：所有 system 同批并发启动，返回 `system_count`
/// - 单线程执行器：同一时刻只有一个 system 能启动，返回 1
fn count_concurrently_started(
    schedule: &mut Schedule,
    system_count: usize,
    wait: Duration,
) -> usize {
    let (started_tx, started_rx) = mpsc::channel::<()>();
    let mut release_txs = Vec::with_capacity(system_count);

    for _ in 0..system_count {
        let (release_tx, release_rx) = mpsc::channel::<()>();
        release_txs.push(release_tx);

        let started_tx = started_tx.clone();
        schedule.add_system(move |_: ()| {
            let _ = started_tx.send(());
            let _ = release_rx.recv();
        });
    }

    let coordinator = thread::spawn(move || {
        let mut started = 0;
        for _ in 0..system_count {
            if started_rx.recv_timeout(wait).is_err() {
                break;
            }
            started += 1;
        }
        // 无论是否等齐都放行，避免测试失败时卡死
        for release_tx in release_txs {
            let _ = release_tx.send(());
        }
        started
    });

    let mut world = World::new();
    schedule.run(&mut world);

    coordinator.join().unwrap()
}

#[test]
fn multithreaded_executor_runs_a_batch_concurrently() {
    let mut schedule = Schedule::default();
    schedule.set_executor(MultiThreadedExecutor);

    let started = count_concurrently_started(&mut schedule, 3, Duration::from_secs(5));

    assert_eq!(started, 3, "同批内互不冲突的 system 应该同时执行");
}

#[test]
fn single_threaded_executor_serializes_a_batch() {
    let mut schedule = Schedule::default();
    schedule.set_executor(SingleThreadedExecutor);

    let started = count_concurrently_started(&mut schedule, 3, Duration::from_millis(250));

    assert_eq!(started, 1, "单线程执行器同一时刻只能运行一个 system");
}

#[test]
fn multithreaded_executor_picks_up_systems_added_between_runs() {
    let mut world = World::new();
    let entity = world.spawn(Position(0));

    let observed = Arc::new(Mutex::new(None));
    let observed_in_reader = observed.clone();

    let integrate = |positions: Query<&mut Position>| {
        for position in positions {
            position.0 += 1;
        }
    };
    let record = move |positions: Query<&Position>| {
        *observed_in_reader.lock().unwrap() =
            Some(positions.map(|position| position.0).sum::<i32>());
    };

    // 不设置执行器，走默认的多线程实现
    let mut schedule = Schedule::default();
    schedule.add_system(integrate);
    schedule.run(&mut world);
    assert_eq!(world.get::<Position>(entity).unwrap().unwrap().0, 1);

    // 计划已缓存，新增的 system 必须在多线程路径上也被执行
    schedule.add_system(record.after(integrate));
    schedule.run(&mut world);

    assert_eq!(world.get::<Position>(entity).unwrap().unwrap().0, 2);
    assert_eq!(*observed.lock().unwrap(), Some(2));
}

#[test]
fn executor_can_be_replaced_between_runs() {
    let log = Arc::new(Mutex::new(Vec::new()));
    let first_log = log.clone();
    let first = move |_: ()| first_log.lock().unwrap().push("first");
    let second_log = log.clone();
    let second = move |_: ()| second_log.lock().unwrap().push("second");

    let mut schedule = Schedule::default();
    schedule.set_executor(SingleThreadedExecutor);
    schedule.add_system(first.clone());
    schedule.add_system(second.after(first.clone()));

    let mut world = World::new();
    schedule.run(&mut world);
    assert_eq!(*log.lock().unwrap(), ["first", "second"]);

    // 换执行器不该重建计划，也不该打乱批间顺序
    schedule.set_executor(MultiThreadedExecutor);
    schedule.run(&mut world);

    assert_eq!(*log.lock().unwrap(), ["first", "second", "first", "second"]);
}

#[test]
fn ordering_accepts_an_already_configured_system_as_argument() {
    let log = Arc::new(Mutex::new(Vec::new()));
    let first_log = log.clone();
    let first = move |_: ()| first_log.lock().unwrap().push("first");
    let second_log = log.clone();
    let second = move |_: ()| second_log.lock().unwrap().push("second");

    // 传入的不是裸闭包，而是配置过的 second，
    // before 必须取到 second 本身的 system 类型，而不是 SystemConfig 的类型
    let ordering_target = second.clone().after(first.clone());

    let mut schedule = Schedule::default();
    schedule.set_executor(SingleThreadedExecutor);
    schedule.add_system(first.before(ordering_target));
    schedule.add_system(second);

    schedule.run(&mut World::new());

    assert_eq!(*log.lock().unwrap(), ["first", "second"]);
}

#[test]
fn ordering_uses_only_the_configured_system_type() {
    let log = Arc::new(Mutex::new(Vec::new()));
    let third_log = log.clone();
    let third = move |_: ()| third_log.lock().unwrap().push("third");
    let second_log = log.clone();
    let second = move |_: ()| second_log.lock().unwrap().push("second");
    let first_log = log.clone();
    let first = move |_: ()| first_log.lock().unwrap().push("first");

    // 参数是 second.before(third)：只记录 first -> second，
    // 参数里那条 second -> third 属于另一份配置，不会跟着生效
    let ordering_target = second.clone().before(third.clone());

    let mut schedule = Schedule::default();
    schedule.set_executor(SingleThreadedExecutor);
    schedule.add_system(first.before(ordering_target));
    schedule.add_system(second);
    schedule.add_system(third);

    schedule.run(&mut World::new());

    let labels = log.lock().unwrap();
    assert_eq!(labels.len(), 3);
    // third 没有任何约束，可以和 first 同批；second 依赖 first，所以在最后
    assert_eq!(labels[2], "second");
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
