#[allow(dead_code)]
mod common;

use std::hint::black_box;

use criterion::{BenchmarkId, Criterion, Throughput, criterion_group, criterion_main};
use recs::{Component as RecsComponent, Entity as RecsEntity, World as RecsWorld};

use common::{
    ITER_COUNTS, PositionScalar, RPosition, RTag0, RTag1, RTag2, RTag3, RToggle, RVelocity,
    SMALL_COUNTS, STRUCTURAL_COUNTS, Scalar, configure_group,
};

#[inline(never)]
fn recs_read<P, V>(world: &mut RecsWorld) -> f64
where
    P: RecsComponent + Scalar,
    V: RecsComponent + Scalar,
{
    let sum = world
        .query::<(&P, &V)>()
        .map(|(position, velocity)| position.value() + velocity.value())
        .sum::<f64>();
    black_box(sum)
}

#[inline(never)]
fn recs_write<P, V>(world: &mut RecsWorld) -> f64
where
    P: RecsComponent + PositionScalar,
    V: RecsComponent + Scalar,
{
    let mut sum = 0.0;
    for (position, velocity) in world.query::<(&mut P, &V)>() {
        position.add_velocity(velocity.value());
        sum += position.value();
    }
    black_box(sum)
}

fn add_tags(world: &mut RecsWorld, entity: RecsEntity, mask: usize) {
    if mask & 1 != 0 {
        world.insert(entity, RTag0).unwrap();
    }
    if mask & 2 != 0 {
        world.insert(entity, RTag1).unwrap();
    }
    if mask & 4 != 0 {
        world.insert(entity, RTag2).unwrap();
    }
    if mask & 8 != 0 {
        world.insert(entity, RTag3).unwrap();
    }
}

fn build_world(
    count: usize,
    velocity_stride: usize,
    fragmented: bool,
) -> (RecsWorld, Vec<RecsEntity>) {
    let mut world = RecsWorld::new();
    let mut entities = Vec::with_capacity(count);
    for i in 0..count {
        let entity = world.spawn(RPosition(i as f64));
        if i % velocity_stride == 0 {
            world.insert(entity, RVelocity(1.0)).unwrap();
        }
        if fragmented {
            add_tags(&mut world, entity, i & 15);
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
        let (mut world, entities) = build_world(count, 1, false);
        drop(entities);
        group.bench_function(BenchmarkId::new("recs_sparse", count), |b| {
            b.iter(|| recs_read::<RPosition, RVelocity>(&mut world));
        });
    }
    group.finish();
}

fn bench_dense_write(c: &mut Criterion) {
    let mut group = c.benchmark_group("query/dense_write");
    configure_group(&mut group);
    for count in ITER_COUNTS {
        group.throughput(Throughput::Elements(count as u64));
        let (mut world, entities) = build_world(count, 1, false);
        drop(entities);
        group.bench_function(BenchmarkId::new("recs_sparse", count), |b| {
            b.iter(|| recs_write::<RPosition, RVelocity>(&mut world));
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
            let (mut world, entities) = build_world(count, stride, false);
            drop(entities);
            group.bench_function(
                BenchmarkId::new(format!("recs_sparse/{}pct", 100 / stride), count),
                |b| b.iter(|| recs_read::<RPosition, RVelocity>(&mut world)),
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
        let (mut world, entities) = build_world(count, 1, true);
        drop(entities);
        group.bench_function(BenchmarkId::new("recs_sparse_16_masks", count), |b| {
            b.iter(|| recs_read::<RPosition, RVelocity>(&mut world));
        });
    }
    group.finish();
}

fn bench_add_remove(c: &mut Criterion) {
    let mut group = c.benchmark_group("component/add_remove");
    configure_group(&mut group);
    for count in STRUCTURAL_COUNTS {
        group.throughput(Throughput::Elements((count * 2) as u64));
        let mut world = RecsWorld::new();
        let entities = (0..count)
            .map(|i| world.spawn(RPosition(i as f64)))
            .collect::<Vec<_>>();
        group.bench_function(BenchmarkId::new("recs_sparse", count), |b| {
            b.iter(|| {
                for (i, &entity) in entities.iter().enumerate() {
                    world.insert(entity, RToggle(i as f64)).unwrap();
                }
                for &entity in &entities {
                    world.remove::<RToggle>(entity).unwrap();
                }
            });
        });
    }
    group.finish();
}

fn recs_spawn(count: usize) -> RecsWorld {
    let mut world = RecsWorld::new();
    for i in 0..count {
        world.spawn((RPosition(i as f64), RVelocity(1.0)));
    }
    world
}

fn bench_spawn(c: &mut Criterion) {
    let mut group = c.benchmark_group("entity/spawn");
    configure_group(&mut group);
    for count in STRUCTURAL_COUNTS {
        group.throughput(Throughput::Elements(count as u64));
        group.bench_function(BenchmarkId::new("recs_sparse_single", count), |b| {
            b.iter(|| black_box(recs_spawn(count)));
        });
    }
    group.finish();
}

fn bench_despawn_spawn(c: &mut Criterion) {
    let mut group = c.benchmark_group("entity/despawn_spawn");
    configure_group(&mut group);
    for count in STRUCTURAL_COUNTS {
        group.throughput(Throughput::Elements((count * 2) as u64));
        let mut world = RecsWorld::new();
        let mut entities = (0..count)
            .map(|i| world.spawn((RPosition(i as f64), RVelocity(1.0))))
            .collect::<Vec<_>>();
        group.bench_function(BenchmarkId::new("recs_sparse", count), |b| {
            b.iter(|| {
                for entity in entities.drain(..) {
                    world.despawn(entity).unwrap();
                }
                for i in 0..count {
                    entities.push(world.spawn((RPosition(i as f64), RVelocity(1.0))));
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
        let (world, entities) = build_world(count, 1, false);
        group.bench_function(BenchmarkId::new("recs_sparse", count), |b| {
            b.iter(|| {
                let sum = order
                    .iter()
                    .map(|&i| {
                        world
                            .get::<RPosition>(entities[i])
                            .unwrap()
                            .unwrap()
                            .value()
                    })
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
