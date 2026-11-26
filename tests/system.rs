use recs::{Component, Query, World};

#[derive(Component)]
struct Position(i32);

#[derive(Component)]
struct Velocity(i32);

#[derive(Component)]
struct Health(i32);

#[test]
fn system_mutates_query_parameter() {
    let mut world = World::new();
    let entity = world.spawn(Position(10));

    world.run_system(|positions: Query<&mut Position>| {
        for position in positions {
            position.0 += 5;
        }
    });

    assert_eq!(world.get::<Position>(entity).unwrap().unwrap().0, 15);
}

#[test]
fn system_accepts_three_query_parameters() {
    let mut world = World::new();
    let entity = world.spawn((Position(1), Velocity(10), Health(100)));
    world.spawn((Position(2), Velocity(20)));

    world.run_system(
        |positions: Query<&mut Position>, velocities: Query<&Velocity>, healths: Query<&Health>| {
            for position in positions {
                position.0 += 1;
            }
            assert_eq!(velocities.map(|velocity| velocity.0).sum::<i32>(), 30);
            assert_eq!(healths.map(|health| health.0).sum::<i32>(), 100);
        },
    );

    assert_eq!(world.get::<Position>(entity).unwrap().unwrap().0, 2);
}

#[test]
#[should_panic(expected = "query access conflict")]
fn system_rejects_conflicting_access_across_parameters() {
    let mut world = World::new();
    world.spawn(Position(1));

    world.run_system(|(_write, _read): (Query<&mut Position>, Query<&Position>)| {});
}
