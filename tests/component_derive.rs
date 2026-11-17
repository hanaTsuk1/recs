use recs::Component;

#[derive(Component)]
struct Position;

#[derive(Component)]
struct Wrapper<T>(T);

#[derive(Component)]
struct Pair<T, U>(T, U);

#[derive(Component)]
struct Bounded<T>
where
    T: Send + Sync,
{
    _value: T,
}

#[derive(Component)]
struct ArrayWrapper<T, const N: usize>([T; N]);

#[test]
fn derive_component_for_generic_types() {
    fn assert_component<T: Component>() {}

    assert_component::<Position>();
    assert_component::<Wrapper<i32>>();
    assert_component::<Pair<i32, f32>>();
    assert_component::<Bounded<i32>>();
    assert_component::<ArrayWrapper<i32, 4>>();
}
