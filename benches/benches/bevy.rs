#[allow(dead_code)]
mod common;

use std::hint::black_box;

use bevy_ecs::{
    component::Component as BevyComponent, entity::Entity as BevyEntity, query::QueryState,
    world::World as BevyWorld,
};
use criterion::{BenchmarkId, Criterion, Throughput, criterion_group, criterion_main};

use common::{
    ITER_COUNTS, PositionScalar, SMALL_COUNTS, SPosition, STRUCTURAL_COUNTS, STag0, STag1, STag2,
    STag3, SToggle, SVelocity, Scalar, TPosition, TTag0, TTag1, TTag2, TTag3, TToggle, TVelocity,
    configure_group,
};

#[inline(never)]
fn bevy_read<P, V>(query: &mut QueryState<(&'static P, &'static V)>, world: &BevyWorld) -> f64
where
    P: BevyComponent + Scalar,
    V: BevyComponent + Scalar,
{
    let sum = query
        .iter(world)
        .map(|(position, velocity)| position.value() + velocity.value())
        .sum::<f64>();
    black_box(sum)
}

#[inline(never)]
fn bevy_write<P, V>(
    query: &mut QueryState<(&'static mut P, &'static V)>,
    world: &mut BevyWorld,
) -> f64
where
    P: BevyComponent<Mutability = bevy_ecs::component::Mutable> + PositionScalar,
    V: BevyComponent + Scalar,
{
    let mut sum = 0.0;
    for (mut position, velocity) in query.iter_mut(world) {
        position.add_velocity(velocity.value());
        sum += position.value();
    }
    black_box(sum)
}

fn add_table_tags(world: &mut BevyWorld, entity: BevyEntity, mask: usize) {
    if mask & 1 != 0 {
        world.entity_mut(entity).insert(TTag0);
    }
    if mask & 2 != 0 {
        world.entity_mut(entity).insert(TTag1);
    }
    if mask & 4 != 0 {
        world.entity_mut(entity).insert(TTag2);
    }
    if mask & 8 != 0 {
        world.entity_mut(entity).insert(TTag3);
    }
}

fn add_sparse_tags(world: &mut BevyWorld, entity: BevyEntity, mask: usize) {
    if mask & 1 != 0 {
        world.entity_mut(entity).insert(STag0);
    }
    if mask & 2 != 0 {
        world.entity_mut(entity).insert(STag1);
    }
    if mask & 4 != 0 {
        world.entity_mut(entity).insert(STag2);
    }
    if mask & 8 != 0 {
        world.entity_mut(entity).insert(STag3);
    }
}

fn build_table(
    count: usize,
    velocity_stride: usize,
    fragmented: bool,
) -> (BevyWorld, Vec<BevyEntity>) {
    let mut world = BevyWorld::new();
    let mut entities = Vec::with_capacity(count);
    for i in 0..count {
        let entity = world.spawn(TPosition(i as f64)).id();
        if i % velocity_stride == 0 {
            world.entity_mut(entity).insert(TVelocity(1.0));
        }
        if fragmented {
            add_table_tags(&mut world, entity, i & 15);
        }
        entities.push(entity);
    }
    (world, entities)
}

fn build_sparse(
    count: usize,
    velocity_stride: usize,
    fragmented: bool,
) -> (BevyWorld, Vec<BevyEntity>) {
    let mut world = BevyWorld::new();
    let mut entities = Vec::with_capacity(count);
    for i in 0..count {
        let entity = world.spawn(SPosition(i as f64)).id();
        if i % velocity_stride == 0 {
            world.entity_mut(entity).insert(SVelocity(1.0));
        }
        if fragmented {
            add_sparse_tags(&mut world, entity, i & 15);
        }
        entities.push(entity);
    }
    (world, entities)
}

fn bench_dense_read(c: &mut Criterion) {
    let mut group = c.benchmark_group("query/dense_read");
    configure_group(&mut group);
    for count in ITER_COUNTS {
        group.throughput(Throughput::Elements(count as u64));

        let (mut world, entities) = build_table(count, 1, false);
        drop(entities);
        let mut query = world.query::<(&TPosition, &TVelocity)>();
        group.bench_function(BenchmarkId::new("bevy_table", count), |b| {
            b.iter(|| bevy_read(&mut query, &world));
        });

        let (mut world, entities) = build_sparse(count, 1, false);
        drop(entities);
        let mut query = world.query::<(&SPosition, &SVelocity)>();
        group.bench_function(BenchmarkId::new("bevy_sparse", count), |b| {
            b.iter(|| bevy_read(&mut query, &world));
        });
    }
    group.finish();
}

fn bench_dense_write(c: &mut Criterion) {
    let mut group = c.benchmark_group("query/dense_write");
    configure_group(&mut group);
    for count in ITER_COUNTS {
        group.throughput(Throughput::Elements(count as u64));

        let (mut world, entities) = build_table(count, 1, false);
        drop(entities);
        let mut query = world.query::<(&mut TPosition, &TVelocity)>();
        group.bench_function(BenchmarkId::new("bevy_table", count), |b| {
            b.iter(|| bevy_write(&mut query, &mut world));
        });

        let (mut world, entities) = build_sparse(count, 1, false);
        drop(entities);
        let mut query = world.query::<(&mut SPosition, &SVelocity)>();
        group.bench_function(BenchmarkId::new("bevy_sparse", count), |b| {
            b.iter(|| bevy_write(&mut query, &mut world));
        });
    }
    group.finish();
}

fn bench_sparse_join(c: &mut Criterion) {
    let mut group = c.benchmark_group("query/sparse_join_read");
    configure_group(&mut group);
    for count in SMALL_COUNTS {
        for stride in [10, 2, 1] {
            let matching = count.div_ceil(stride);
            group.throughput(Throughput::Elements(matching as u64));

            let (mut world, entities) = build_table(count, stride, false);
            drop(entities);
            let mut query = world.query::<(&TPosition, &TVelocity)>();
            group.bench_function(
                BenchmarkId::new(format!("bevy_table/{}pct", 100 / stride), count),
                |b| b.iter(|| bevy_read(&mut query, &world)),
            );

            let (mut world, entities) = build_sparse(count, stride, false);
            drop(entities);
            let mut query = world.query::<(&SPosition, &SVelocity)>();
            group.bench_function(
                BenchmarkId::new(format!("bevy_sparse/{}pct", 100 / stride), count),
                |b| b.iter(|| bevy_read(&mut query, &world)),
            );
        }
    }
    group.finish();
}

fn bench_fragmented(c: &mut Criterion) {
    let mut group = c.benchmark_group("query/fragmented_read");
    configure_group(&mut group);
    for count in SMALL_COUNTS {
        group.throughput(Throughput::Elements(count as u64));

        let (mut world, entities) = build_table(count, 1, true);
        drop(entities);
        let mut query = world.query::<(&TPosition, &TVelocity)>();
        group.bench_function(BenchmarkId::new("bevy_table_16_archetypes", count), |b| {
            b.iter(|| bevy_read(&mut query, &world));
        });

        let (mut world, entities) = build_sparse(count, 1, true);
        drop(entities);
        let mut query = world.query::<(&SPosition, &SVelocity)>();
        group.bench_function(BenchmarkId::new("bevy_sparse_16_archetypes", count), |b| {
            b.iter(|| bevy_read(&mut query, &world));
        });
    }
    group.finish();
}

fn bench_add_remove(c: &mut Criterion) {
    let mut group = c.benchmark_group("component/add_remove");
    configure_group(&mut group);
    for count in STRUCTURAL_COUNTS {
        group.throughput(Throughput::Elements((count * 2) as u64));

        let mut world = BevyWorld::new();
        let entities = (0..count)
            .map(|i| world.spawn(TPosition(i as f64)).id())
            .collect::<Vec<_>>();
        group.bench_function(BenchmarkId::new("bevy_table", count), |b| {
            b.iter(|| {
                for (i, &entity) in entities.iter().enumerate() {
                    world.entity_mut(entity).insert(TToggle(i as f64));
                }
                for &entity in &entities {
                    world.entity_mut(entity).remove::<TToggle>();
                }
            });
        });

        let mut world = BevyWorld::new();
        let entities = (0..count)
            .map(|i| world.spawn(SPosition(i as f64)).id())
            .collect::<Vec<_>>();
        group.bench_function(BenchmarkId::new("bevy_sparse", count), |b| {
            b.iter(|| {
                for (i, &entity) in entities.iter().enumerate() {
                    world.entity_mut(entity).insert(SToggle(i as f64));
                }
                for &entity in &entities {
                    world.entity_mut(entity).remove::<SToggle>();
                }
            });
        });
    }
    group.finish();
}

fn table_spawn(count: usize) -> BevyWorld {
    let mut world = BevyWorld::new();
    for i in 0..count {
        world.spawn((TPosition(i as f64), TVelocity(1.0)));
    }
    world
}

fn sparse_spawn(count: usize) -> BevyWorld {
    let mut world = BevyWorld::new();
    for i in 0..count {
        world.spawn((SPosition(i as f64), SVelocity(1.0)));
    }
    world
}

fn table_spawn_batch(count: usize) -> BevyWorld {
    let mut world = BevyWorld::new();
    world
        .spawn_batch((0..count).map(|i| (TPosition(i as f64), TVelocity(1.0))))
        .count();
    world
}

fn sparse_spawn_batch(count: usize) -> BevyWorld {
    let mut world = BevyWorld::new();
    world
        .spawn_batch((0..count).map(|i| (SPosition(i as f64), SVelocity(1.0))))
        .count();
    world
}

fn bench_spawn(c: &mut Criterion) {
    let mut group = c.benchmark_group("entity/spawn");
    configure_group(&mut group);
    for count in STRUCTURAL_COUNTS {
        group.throughput(Throughput::Elements(count as u64));
        group.bench_function(BenchmarkId::new("bevy_table_single", count), |b| {
            b.iter(|| black_box(table_spawn(count)));
        });
        group.bench_function(BenchmarkId::new("bevy_sparse_single", count), |b| {
            b.iter(|| black_box(sparse_spawn(count)));
        });
        group.bench_function(BenchmarkId::new("bevy_table_batch", count), |b| {
            b.iter(|| black_box(table_spawn_batch(count)));
        });
        group.bench_function(BenchmarkId::new("bevy_sparse_batch", count), |b| {
            b.iter(|| black_box(sparse_spawn_batch(count)));
        });
    }
    group.finish();
}

fn bench_despawn_spawn(c: &mut Criterion) {
    let mut group = c.benchmark_group("entity/despawn_spawn");
    configure_group(&mut group);
    for count in STRUCTURAL_COUNTS {
        group.throughput(Throughput::Elements((count * 2) as u64));

        let mut world = BevyWorld::new();
        let mut entities = (0..count)
            .map(|i| world.spawn((TPosition(i as f64), TVelocity(1.0))).id())
            .collect::<Vec<_>>();
        group.bench_function(BenchmarkId::new("bevy_table", count), |b| {
            b.iter(|| {
                for entity in entities.drain(..) {
                    world.despawn(entity);
                }
                for i in 0..count {
                    entities.push(world.spawn((TPosition(i as f64), TVelocity(1.0))).id());
                }
                black_box(entities.len())
            });
        });

        let mut world = BevyWorld::new();
        let mut entities = (0..count)
            .map(|i| world.spawn((SPosition(i as f64), SVelocity(1.0))).id())
            .collect::<Vec<_>>();
        group.bench_function(BenchmarkId::new("bevy_sparse", count), |b| {
            b.iter(|| {
                for entity in entities.drain(..) {
                    world.despawn(entity);
                }
                for i in 0..count {
                    entities.push(world.spawn((SPosition(i as f64), SVelocity(1.0))).id());
                }
                black_box(entities.len())
            });
        });
    }
    group.finish();
}

fn bench_random_get(c: &mut Criterion) {
    let mut group = c.benchmark_group("entity/random_get");
    configure_group(&mut group);
    for count in SMALL_COUNTS {
        group.throughput(Throughput::Elements(count as u64));
        let order = (0..count)
            .map(|i| i.wrapping_mul(97) % count)
            .collect::<Vec<_>>();

        let (world, entities) = build_table(count, 1, false);
        group.bench_function(BenchmarkId::new("bevy_table", count), |b| {
            b.iter(|| {
                let sum = order
                    .iter()
                    .map(|&i| world.get::<TPosition>(entities[i]).unwrap().value())
                    .sum::<f64>();
                black_box(sum)
            });
        });

        let (world, entities) = build_sparse(count, 1, false);
        group.bench_function(BenchmarkId::new("bevy_sparse", count), |b| {
            b.iter(|| {
                let sum = order
                    .iter()
                    .map(|&i| world.get::<SPosition>(entities[i]).unwrap().value())
                    .sum::<f64>();
                black_box(sum)
            });
        });
    }
    group.finish();
}

criterion_group!(
    benches,
    bench_dense_read,
    bench_dense_write,
    bench_sparse_join,
    bench_fragmented,
    bench_add_remove,
    bench_spawn,
    bench_despawn_spawn,
    bench_random_get,
);
criterion_main!(benches);
