use recs::{Component, World};

#[derive(Component)]
struct Position(i32);

#[derive(Component)]
struct Velocity(i32);

#[derive(Component)]
struct Health(i32);

#[derive(Component)]
struct Armor(i32);

#[test]
fn read_query_returns_all_matching_components() {
    let mut world = World::new();
    world.spawn(Position(10));
    world.spawn(Position(20));
    world.spawn(Velocity(30));

    let mut values = world
        .query::<&Position>()
        .map(|position| position.0)
        .collect::<Vec<_>>();
    values.sort_unstable();

    assert_eq!(values, [10, 20]);
}

#[test]
fn write_query_mutates_components() {
    let mut world = World::new();
    let first = world.spawn(Position(10));
    let second = world.spawn(Position(20));

    for position in world.query::<&mut Position>() {
        position.0 += 1;
    }

    assert_eq!(world.get::<Position>(first).unwrap().unwrap().0, 11);
    assert_eq!(world.get::<Position>(second).unwrap().unwrap().0, 21);
}

#[test]
fn tuple_read_query_joins_components_by_entity() {
    let mut world = World::new();
    world.spawn((Position(1), Velocity(10)));
    world.spawn(Position(2));
    world.spawn((Position(3), Velocity(30)));

    let mut rows = world
        .query::<(&Position, &Velocity)>()
        .map(|(position, velocity)| (position.0, velocity.0))
        .collect::<Vec<_>>();
    rows.sort_unstable();

    assert_eq!(rows, [(1, 10), (3, 30)]);
}

#[test]
fn three_component_query_joins_by_entity_and_skips_incomplete_rows() {
    let mut world = World::new();
    world.spawn((Position(1), Velocity(10), Health(100)));
    world.spawn((Position(2), Velocity(20)));
    world.spawn((Velocity(30), Health(300)));
    world.spawn((Position(4), Health(400)));
    world.spawn((Position(5), Velocity(50), Health(500)));

    let mut rows = world
        .query::<(&Position, &Velocity, &Health)>()
        .map(|(position, velocity, health)| (position.0, velocity.0, health.0))
        .collect::<Vec<_>>();
    rows.sort_unstable();

    assert_eq!(rows, [(1, 10, 100), (5, 50, 500)]);
}

#[test]
fn four_component_query_joins_and_mutates_matching_entities() {
    let mut world = World::new();
    let matched = world.spawn((Position(1), Velocity(10), Health(100), Armor(1000)));
    let missing_armor = world.spawn((Position(2), Velocity(20), Health(200)));
    let missing_health = world.spawn((Position(3), Velocity(30), Armor(3000)));

    for (position, velocity, health, armor) in
        world.query::<(&mut Position, &Velocity, &Health, &Armor)>()
    {
        position.0 += velocity.0 + health.0 + armor.0;
    }

    assert_eq!(world.get::<Position>(matched).unwrap().unwrap().0, 1111);
    assert_eq!(world.get::<Position>(missing_armor).unwrap().unwrap().0, 2);
    assert_eq!(world.get::<Position>(missing_health).unwrap().unwrap().0, 3);
}

#[test]
fn tuple_query_skips_nonmatching_entities_and_continues() {
    let mut world = World::new();
    world.spawn((Position(1), Velocity(10)));
    world.spawn(Velocity(20));
    world.spawn((Position(3), Velocity(30)));
    world.spawn(Position(4));
    world.spawn(Position(5));

    let mut rows = world
        .query::<(&Position, &Velocity)>()
        .map(|(position, velocity)| (position.0, velocity.0))
        .collect::<Vec<_>>();
    rows.sort_unstable();

    assert_eq!(rows, [(1, 10), (3, 30)]);
}

#[test]
fn tuple_query_joins_correctly_after_component_removal_reorders_storage() {
    let mut world = World::new();
    world.spawn((Position(1), Velocity(10)));
    let removed = world.spawn((Position(2), Velocity(20)));
    world.spawn((Position(3), Velocity(30)));

    assert_eq!(world.remove::<Position>(removed).unwrap().unwrap().0, 2);

    let mut rows = world
        .query::<(&Position, &Velocity)>()
        .map(|(position, velocity)| (position.0, velocity.0))
        .collect::<Vec<_>>();
    rows.sort_unstable();

    assert_eq!(rows, [(1, 10), (3, 30)]);
}

#[test]
fn tuple_query_does_not_return_components_from_a_reused_entity_slot() {
    let mut world = World::new();
    let old = world.spawn((Position(1), Velocity(10)));
    world.despawn(old).unwrap();
    let replacement = world.spawn((Position(2), Velocity(20)));

    assert_eq!(old.index(), replacement.index());

    let rows = world
        .query::<(&Position, &Velocity)>()
        .map(|(position, velocity)| (position.0, velocity.0))
        .collect::<Vec<_>>();

    assert_eq!(rows, [(2, 20)]);
}

#[test]
fn tuple_write_query_only_mutates_entities_with_both_components() {
    let mut world = World::new();
    let matched = world.spawn((Position(1), Velocity(10)));
    let position_only = world.spawn(Position(2));
    let velocity_only = world.spawn(Velocity(30));

    for (position, velocity) in world.query::<(&mut Position, &Velocity)>() {
        position.0 += velocity.0;
    }

    assert_eq!(world.get::<Position>(matched).unwrap().unwrap().0, 11);
    assert_eq!(world.get::<Position>(position_only).unwrap().unwrap().0, 2);
    assert_eq!(world.get::<Velocity>(velocity_only).unwrap().unwrap().0, 30);
}

#[test]
fn query_for_missing_storage_is_empty() {
    let mut world = World::new();
    world.spawn(Position(1));

    assert_eq!(world.query::<&Health>().count(), 0);
    assert_eq!(world.query::<(&Position, &Health)>().count(), 0);
}

#[test]
fn tuple_query_is_empty_when_storages_have_no_common_entity() {
    let mut world = World::new();
    world.spawn(Position(1));
    world.spawn(Velocity(10));

    assert_eq!(world.query::<(&Position, &Velocity)>().count(), 0);
}

#[test]
fn repeated_read_access_to_same_component_is_allowed() {
    let mut world = World::new();
    world.spawn(Position(7));

    let values = world
        .query::<(&Position, &Position)>()
        .map(|(first, second)| (first.0, second.0))
        .collect::<Vec<_>>();

    assert_eq!(values, [(7, 7)]);
}

#[test]
#[should_panic(expected = "query access conflict")]
fn read_write_access_to_same_component_is_rejected() {
    let mut world = World::new();
    world.spawn(Position(7));

    let _query = world.query::<(&Position, &mut Position)>();
}

#[test]
#[should_panic(expected = "query access conflict")]
fn write_read_access_to_same_component_is_rejected() {
    let mut world = World::new();
    world.spawn(Position(7));

    let _query = world.query::<(&mut Position, &Position)>();
}

#[test]
#[should_panic(expected = "query access conflict")]
fn repeated_write_access_to_same_component_is_rejected() {
    let mut world = World::new();
    world.spawn(Position(7));

    let _query = world.query::<(&mut Position, &mut Position)>();
}

#[test]
#[should_panic(expected = "query access conflict")]
fn nested_tuple_access_conflict_is_rejected() {
    let mut world = World::new();
    world.spawn((Position(7), Velocity(3)));

    let _query = world.query::<((&mut Position, &Velocity), &Position)>();
}
