use std::sync::{Arc, Mutex};

use recs::{Query, Resource, World};

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
