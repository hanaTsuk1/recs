use std::sync::{Arc, Mutex};

use recs::{
    IntoSystemConfig, Query, Res, ResMut, Resource, Schedule, SingleThreadedExecutor, World,
};

#[derive(Resource, Debug, PartialEq)]
struct Config {
    value: i32,
}

#[derive(Resource, Debug, PartialEq)]
struct Counter(u32);

#[test]
fn insert_then_remove_returns_the_resource() {
    let mut world = World::new();
    world.insert_resource(Config { value: 1 }).unwrap();

    assert_eq!(
        world.remove_resource::<Config>().unwrap(),
        Some(Config { value: 1 })
    );
}

#[test]
fn get_resource_reads_and_mutates() {
    let mut world = World::new();
    assert_eq!(world.get_resource::<Config>().unwrap(), None);

    world.insert_resource(Config { value: 1 }).unwrap();
    assert_eq!(
        world.get_resource::<Config>().unwrap(),
        Some(&Config { value: 1 })
    );

    // 通过可变引用改值，再读回来
    world.get_resource_mut::<Config>().unwrap().unwrap().value += 1;
    assert_eq!(world.get_resource::<Config>().unwrap().unwrap().value, 2);

    world.remove_resource::<Config>().unwrap();
    assert_eq!(world.get_resource::<Config>().unwrap(), None);
    assert_eq!(world.get_resource_mut::<Config>().unwrap(), None);
}

#[test]
fn removing_a_missing_resource_returns_none() {
    let mut world = World::new();
    assert_eq!(world.remove_resource::<Config>().unwrap(), None);

    world.insert_resource(Config { value: 2 }).unwrap();
    world.remove_resource::<Config>().unwrap();
    // 已经移除过，再移除一次仍然是 None
    assert_eq!(world.remove_resource::<Config>().unwrap(), None);
}

#[test]
fn inserting_again_replaces_the_value() {
    let mut world = World::new();
    world.insert_resource(Config { value: 1 }).unwrap();
    world.insert_resource(Config { value: 2 }).unwrap();

    // 只保留一个资源实体，不会因为重复插入而多出一份
    assert_eq!(world.query::<&Config>().count(), 1);
    assert_eq!(
        world.remove_resource::<Config>().unwrap(),
        Some(Config { value: 2 })
    );
}

#[test]
fn resources_of_different_types_coexist() {
    let mut world = World::new();
    world.insert_resource(Config { value: 10 }).unwrap();
    world.insert_resource(Counter(3)).unwrap();

    // 移除一种资源不影响另一种
    assert_eq!(
        world.remove_resource::<Counter>().unwrap(),
        Some(Counter(3))
    );
    assert_eq!(
        world.remove_resource::<Config>().unwrap(),
        Some(Config { value: 10 })
    );
    assert_eq!(world.remove_resource::<Counter>().unwrap(), None);
}

#[test]
fn removing_then_inserting_again_works() {
    let mut world = World::new();
    world.insert_resource(Config { value: 1 }).unwrap();
    world.remove_resource::<Config>().unwrap();
    world.insert_resource(Config { value: 2 }).unwrap();

    assert_eq!(world.query::<&Config>().count(), 1);
    assert_eq!(
        world.remove_resource::<Config>().unwrap(),
        Some(Config { value: 2 })
    );
}

#[test]
fn resource_is_visible_to_queries() {
    // 当前实现把 resource 存成隐藏实体上的组件，所以 Query 也能读到。
    // 以后加了 Res/ResMut 或者打算把资源藏起来，这个用例要跟着改。
    let mut world = World::new();
    world.insert_resource(Config { value: 9 }).unwrap();

    assert_eq!(world.query::<&Config>().count(), 1);
    assert_eq!(
        world.query::<&Config>().next().map(|config| config.value),
        Some(9)
    );

    world.remove_resource::<Config>().unwrap();
    assert_eq!(world.query::<&Config>().count(), 0);
}

#[test]
fn resource_is_readable_from_a_system() {
    let mut world = World::new();
    world.insert_resource(Config { value: 21 }).unwrap();

    let observed = Arc::new(Mutex::new(None));
    let observed_in_system = observed.clone();
    world.run_system(move |mut configs: Query<&Config>| {
        *observed_in_system.lock().unwrap() = configs.next().map(|config| config.value);
    });

    assert_eq!(*observed.lock().unwrap(), Some(21));
}

#[test]
fn resources_survive_unrelated_entity_changes() {
    let mut world = World::new();
    world.insert_resource(Config { value: 5 }).unwrap();

    let first = world.spawn(Counter(1));
    let second = world.spawn(Counter(2));
    world.despawn(first).unwrap();
    world.despawn(second).unwrap();

    // 普通实体的增删不影响资源实体
    assert_eq!(
        world.remove_resource::<Config>().unwrap(),
        Some(Config { value: 5 })
    );
}

#[test]
fn res_reads_resource_in_a_system() {
    let mut world = World::new();
    world.insert_resource(Config { value: 7 }).unwrap();

    let observed = Arc::new(Mutex::new(None));
    let observed_in_system = observed.clone();
    world.run_system(move |config: Res<Config>| {
        *observed_in_system.lock().unwrap() = Some(config.value);
    });

    assert_eq!(*observed.lock().unwrap(), Some(7));
}

#[test]
fn resmut_mutates_resource_in_a_system() {
    let mut world = World::new();
    world.insert_resource(Config { value: 1 }).unwrap();

    world.run_system(|mut config: ResMut<Config>| {
        config.value += 1;
    });

    assert_eq!(
        world.get_resource::<Config>().unwrap(),
        Some(&Config { value: 2 })
    );
}

#[test]
fn res_and_resmut_for_different_resources_work_together() {
    let mut world = World::new();
    world.insert_resource(Config { value: 10 }).unwrap();
    world.insert_resource(Counter(1)).unwrap();

    world.run_system(|config: Res<Config>, mut counter: ResMut<Counter>| {
        counter.0 += config.value as u32;
    });

    assert_eq!(world.get_resource::<Counter>().unwrap(), Some(&Counter(11)));
}

#[test]
fn changes_made_by_resmut_are_visible_to_later_systems() {
    let mut world = World::new();
    world.insert_resource(Config { value: 0 }).unwrap();

    let observed = Arc::new(Mutex::new(None));
    let observed_in_reader = observed.clone();

    let increment = |mut config: ResMut<Config>| config.value += 1;
    let read = move |config: Res<Config>| {
        *observed_in_reader.lock().unwrap() = Some(config.value);
    };

    let mut schedule = Schedule::default();
    schedule.set_executor(SingleThreadedExecutor);
    schedule.add_system(increment);
    schedule.add_system(read.after(increment));

    schedule.run(&mut world);

    assert_eq!(*observed.lock().unwrap(), Some(1));
}

#[test]
#[should_panic(expected = "resource does not exist")]
fn res_without_inserted_resource_panics() {
    let mut world = World::new();
    world.run_system(|_config: Res<Config>| {});
}

#[test]
fn query_can_mutate_a_resource_through_its_hidden_entity() {
    let mut world = World::new();
    world.insert_resource(Config { value: 1 }).unwrap();

    // 资源是隐藏实体上的组件，所以 Query<&mut Config> 能改到它；
    // 这也是 Res / Query 必须按同一类型声明冲突的原因
    world.run_system(|configs: Query<&mut Config>| {
        for config in configs {
            config.value += 1;
        }
    });

    assert_eq!(
        world.get_resource::<Config>().unwrap(),
        Some(&Config { value: 2 })
    );
}

#[test]
#[should_panic(expected = "query access conflict")]
fn system_with_query_and_resmut_of_the_same_resource_panics() {
    let mut world = World::new();
    world.insert_resource(Config { value: 0 }).unwrap();

    let mut schedule = Schedule::default();
    schedule.set_executor(SingleThreadedExecutor);
    // 同一个资源既被 Query 可变借用、又被 ResMut 可变借用，system 自身就冲突
    schedule.add_system(|(_configs, _config): (Query<&mut Config>, ResMut<Config>)| {});

    schedule.run(&mut world);
}

#[test]
fn system_with_query_and_res_of_the_same_resource_is_allowed() {
    let mut world = World::new();
    world.insert_resource(Config { value: 5 }).unwrap();

    let observed = Arc::new(Mutex::new(None));
    let observed_in_system = observed.clone();

    let mut schedule = Schedule::default();
    schedule.set_executor(SingleThreadedExecutor);
    // 同一个资源只读两次，可以放在同一个 system 里
    schedule.add_system(move |(configs, config): (Query<&Config>, Res<Config>)| {
        let queried: i32 = configs.map(|config| config.value).sum();
        *observed_in_system.lock().unwrap() = Some((queried, config.value));
    });

    schedule.run(&mut world);

    assert_eq!(*observed.lock().unwrap(), Some((5, 5)));
}

#[test]
fn resmut_and_query_writes_to_the_same_resource_are_serialized() {
    let mut world = World::new();
    world.insert_resource(Config { value: 1 }).unwrap();

    let set = |mut config: ResMut<Config>| config.value = 10;
    let bump = |configs: Query<&mut Config>| {
        for config in configs {
            config.value += 1;
        }
    };

    let mut schedule = Schedule::default();
    schedule.set_executor(SingleThreadedExecutor);
    schedule.add_system(set);
    // 两个都是写，必须拆批；after 保证先 set 再 bump
    schedule.add_system(bump.after(set));

    schedule.run(&mut world);

    // 如果两者被放进同一批并发执行，结果会随执行顺序丢失更新
    assert_eq!(
        world.get_resource::<Config>().unwrap(),
        Some(&Config { value: 11 })
    );
}
