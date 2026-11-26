use std::{
    any::{TypeId, type_name},
    collections::{HashMap, HashSet},
};

use crate::{
    World,
    access::Access,
    error::{ErrorHandler, RecsError, RecsResult},
    executor::{MultiThreadedExecutor, SystemExecutor},
    system::{FunctionSystem, IntoSystem, System, SystemParam, SystemParamFunction},
};

pub struct Schedule {
    graph: DependencyGraph,
    plan: Vec<Vec<SystemId>>,
    executor: Box<dyn SystemExecutor>,
    dirty: bool,
    error_handler: ErrorHandler,
}

impl Default for Schedule {
    fn default() -> Self {
        Self {
            graph: Default::default(),
            plan: vec![],
            executor: Box::new(MultiThreadedExecutor),
            dirty: false,
            error_handler: |error| panic!("failed to run: {:?}", error),
        }
    }
}

impl Schedule {
    pub fn add_system<T: System>(&mut self, system_config: impl IntoSystemConfig<T>) -> SystemId {
        let system_config = system_config.into_config();
        let id = self.graph.insert(system_config);
        self.dirty = true;
        id
    }

    pub fn run(&mut self, world: &mut World) {
        if self.dirty {
            self.build_plan();
            self.dirty = false;
        }

        let Self {
            graph,
            plan,
            executor,
            error_handler,
            ..
        } = self;

        let mut plan_view = SystemPlan {
            systems: &mut graph.systems,
            batches: plan,
        };
        executor.run(&mut plan_view, world, *error_handler);
    }

    fn build_plan(&mut self) {
        let plan = match self.graph.build_plan() {
            Ok(plan) => plan,
            Err(err) => panic!("failed to build schedule plan: {:?}", err),
        };
        self.plan = plan;
    }

    pub fn set_executor(&mut self, executor: impl SystemExecutor + 'static) {
        self.executor = Box::new(executor);
    }

    pub fn set_error_handler(&mut self, handler: ErrorHandler) {
        self.error_handler = handler;
    }
}

pub struct SystemConfig<T: System> {
    system: T,
    system_type_id: TypeId,
    system_type_name: &'static str,
    before: Vec<SystemConfigOrderItem>,
    after: Vec<SystemConfigOrderItem>,
}

struct SystemConfigOrderItem {
    type_id: TypeId,
    type_name: &'static str,
}

impl<T: System> SystemConfig<T> {
    fn before<S: System, C: IntoSystemConfig<S>>(mut self, config: C) -> SystemConfig<T> {
        let config = config.into_config();
        self.before.push(SystemConfigOrderItem {
            type_id: config.system_type_id,
            type_name: config.system_type_name,
        });
        self
    }

    fn after<S: System, C: IntoSystemConfig<S>>(mut self, config: C) -> SystemConfig<T> {
        let config = config.into_config();
        self.after.push(SystemConfigOrderItem {
            type_id: config.system_type_id,
            type_name: config.system_type_name,
        });
        self
    }
}

pub trait IntoSystemConfig<T>
where
    Self: Sized + 'static,
    T: System,
{
    fn into_config(self) -> SystemConfig<T>;

    fn before<S: System, C: IntoSystemConfig<S>>(self, config: C) -> SystemConfig<T> {
        self.into_config().before(config)
    }

    fn after<S: System, C: IntoSystemConfig<S>>(self, config: C) -> SystemConfig<T> {
        self.into_config().after(config)
    }
}

impl<T: System> IntoSystemConfig<T> for SystemConfig<T> {
    fn into_config(self) -> SystemConfig<T> {
        self
    }
}

impl<T, P: SystemParam + 'static> IntoSystemConfig<FunctionSystem<T, fn(P)>> for T
where
    for<'a> &'a mut T: FnMut(P),
    T: SystemParamFunction<fn(P)> + Send + 'static,
{
    fn into_config(self) -> SystemConfig<FunctionSystem<T, fn(P)>> {
        SystemConfig {
            system: T::into_system(self),
            system_type_id: TypeId::of::<T>(),
            system_type_name: type_name::<T>(),
            before: vec![],
            after: vec![],
        }
    }
}

pub struct SystemPlan<'a> {
    systems: &'a mut [Box<dyn System>],
    batches: &'a [Vec<SystemId>],
}

impl SystemPlan<'_> {
    pub fn batch_count(&self) -> usize {
        self.batches.len()
    }

    pub fn batch_systems(&mut self, index: usize) -> Vec<&mut dyn System> {
        let Self { systems, batches } = self;

        // 同一批次里的下标互不相同（每个 system 只属于一个批次），
        // 但编译器不知道这件事，所以用 split_at_mut 切成互不重叠的片段。
        let mut ids: Vec<usize> = batches[index].iter().map(|id| id.index()).collect();
        ids.sort_unstable();

        let mut rest: &mut [Box<dyn System>] = systems;
        let mut batch = Vec::with_capacity(ids.len());
        let mut offset = 0;
        for id in ids {
            let (_, tail) = std::mem::take(&mut rest).split_at_mut(id - offset);
            let (head, tail) = tail.split_at_mut(1);
            batch.push(head[0].as_mut());
            rest = tail;
            offset = id + 1;
        }
        batch
    }
}

#[derive(Default)]
struct DependencyGraph {
    systems: Vec<Box<dyn System>>,
    system_types: HashMap<TypeId, SystemSetId>,
    system_sets: HashMap<SystemSetId, HashSet<SystemId>>,
    pending_dependencies: Vec<PendingDependency>,
    dependencies: HashMap<NodeId, HashSet<NodeId>>,
    next_set_id: u32,
}

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Clone, Copy)]
pub struct SystemId(u32);

impl SystemId {
    fn index(self) -> usize {
        self.0 as usize
    }
}

#[derive(PartialEq, Eq, Hash, Clone, Copy)]
pub struct SystemSetId(u32);

#[derive(PartialEq, Eq, Hash, Clone, Copy)]
pub enum NodeId {
    System(SystemId),
    Set(SystemSetId),
}

struct PendingDependency {
    system_id: SystemId,
    before: Vec<SystemConfigOrderItem>,
    after: Vec<SystemConfigOrderItem>,
}

impl DependencyGraph {
    fn insert<T: System>(&mut self, config: SystemConfig<T>) -> SystemId {
        let system = config.system;
        let system_id = SystemId(self.systems.len() as u32);

        let type_id = config.system_type_id;
        self.systems.push(Box::new(system));

        let system_set_id = if let Some(id) = self.system_types.get(&type_id) {
            *id
        } else {
            let id = SystemSetId(self.next_set_id);
            self.next_set_id += 1;
            self.system_types.insert(type_id, id);
            id
        };

        self.system_sets
            .entry(system_set_id)
            .or_default()
            .insert(system_id);

        self.pending_dependencies.push(PendingDependency {
            system_id,
            before: config.before,
            after: config.after,
        });

        system_id
    }

    fn build_dependencies(&mut self) -> RecsResult<()> {
        let mut dependencies = HashMap::<NodeId, HashSet<NodeId>>::new();

        for system_id in 0..self.systems.len() {
            dependencies
                .entry(NodeId::System(SystemId(system_id as u32)))
                .or_default();
        }

        for set_id in self.system_sets.keys() {
            dependencies.entry(NodeId::Set(*set_id)).or_default();
        }

        for PendingDependency {
            system_id,
            before,
            after,
        } in &self.pending_dependencies
        {
            let node_id = NodeId::System(*system_id);
            for item in before {
                let system_set_id = self.get_system_set_id(item)?;
                dependencies
                    .entry(node_id)
                    .or_default()
                    .insert(NodeId::Set(system_set_id));
            }
            for item in after {
                let system_set_id = self.get_system_set_id(item)?;
                dependencies
                    .entry(NodeId::Set(system_set_id))
                    .or_default()
                    .insert(node_id);
            }
        }

        self.dependencies = dependencies;

        Ok(())
    }

    fn build_system_dependencies(&self) -> Vec<Vec<SystemId>> {
        let mut dependencies = vec![Vec::new(); self.systems.len()];

        for (from, targets) in &self.dependencies {
            for to in targets {
                match (from, to) {
                    (NodeId::System(a), NodeId::System(b)) => dependencies[a.index()].push(*b),

                    (NodeId::System(a), NodeId::Set(set)) => {
                        dependencies[a.index()].extend(self.system_sets[set].iter().copied());
                    }

                    (NodeId::Set(set), NodeId::System(b)) => {
                        for a in &self.system_sets[set] {
                            dependencies[a.index()].push(*b);
                        }
                    }

                    (NodeId::Set(a), NodeId::Set(b)) => {
                        for from in &self.system_sets[a] {
                            dependencies[from.index()].extend(self.system_sets[b].iter().copied());
                        }
                    }
                }
            }
        }

        // 排序去重：边的顺序决定下一轮 ready 的顺序，必须确定
        for targets in &mut dependencies {
            targets.sort_unstable();
            targets.dedup();
        }

        dependencies
    }

    fn get_system_set_id(&self, item: &SystemConfigOrderItem) -> RecsResult<SystemSetId> {
        self.system_types
            .get(&item.type_id)
            .copied()
            .ok_or(RecsError::NoSuchSystem {
                name: item.type_name,
            })
    }

    fn build_batches(&self, dependencies: &[Vec<SystemId>]) -> RecsResult<Vec<Vec<SystemId>>> {
        let mut in_degrees = vec![0u32; self.systems.len()];

        for targets in dependencies {
            for target in targets {
                in_degrees[target.index()] += 1;
            }
        }

        // 按 SystemId（注册顺序）升序初始化，批次划分才能复现
        let mut ready: Vec<SystemId> = (0..self.systems.len())
            .map(|system_id| SystemId(system_id as u32))
            .filter(|system_id| in_degrees[system_id.index()] == 0)
            .collect();

        let mut batches = Vec::new();
        let mut processed = 0;

        while !ready.is_empty() {
            let mut batch = Vec::new();
            let mut deferred = Vec::new();

            // 从当前 ready 中贪心选择兼容的 System
            for candidate in ready.drain(..) {
                let compatible = batch
                    .iter()
                    .all(|existing| self.can_run_together(candidate, *existing));

                if compatible {
                    batch.push(candidate);
                } else {
                    deferred.push(candidate);
                }
            }

            // 当前 Batch 已确定
            processed += batch.len();

            // 移除当前 Batch 的出边
            for system_id in &batch {
                for target in &dependencies[system_id.index()] {
                    let degree = &mut in_degrees[target.index()];
                    *degree -= 1;

                    if *degree == 0 {
                        deferred.push(*target);
                    }
                }
            }

            batches.push(batch);

            // 新就绪的 system 来自边表顺序，排序后下一轮的 ready 才确定
            deferred.sort_unstable();
            ready = deferred;
        }

        // 环检测
        if processed != self.systems.len() {
            return Err(RecsError::CircularDependency);
        }

        Ok(batches)
    }

    fn can_run_together(&self, a: SystemId, b: SystemId) -> bool {
        let a = &self.systems[a.index()];
        let b = &self.systems[b.index()];
        let mut access = Access::default();
        a.init_access(&mut access);
        b.init_access(&mut access);
        access.validate().is_ok()
    }

    fn build_plan(&mut self) -> RecsResult<Vec<Vec<SystemId>>> {
        self.build_dependencies()?;
        let dependencies = self.build_system_dependencies();
        self.build_batches(&dependencies)
    }
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};

    use super::*;
    use crate::{Component, Query, world::WorldCell};

    #[derive(Component)]
    struct Position(i32);

    #[derive(Component)]
    struct Velocity(i32);

    #[test]
    fn plan_groups_independent_systems_in_a_single_batch() {
        let mut schedule = Schedule::default();
        schedule.add_system(|_: ()| {});
        schedule.add_system(|_: ()| {});
        schedule.add_system(|_: ()| {});

        schedule.build_plan();

        assert_eq!(schedule.plan.len(), 1);
        assert_eq!(schedule.plan[0].len(), 3);
    }

    #[test]
    fn plan_groups_systems_with_disjoint_accesses_in_a_single_batch() {
        let mut schedule = Schedule::default();
        schedule.add_system(|_: Query<&mut Position>| {});
        schedule.add_system(|_: Query<&mut Velocity>| {});

        schedule.build_plan();

        assert_eq!(schedule.plan.len(), 1);
        assert_eq!(schedule.plan[0].len(), 2);
    }

    #[test]
    fn plan_runs_readers_of_the_same_component_in_a_single_batch() {
        let mut schedule = Schedule::default();
        schedule.add_system(|_: Query<&Position>| {});
        schedule.add_system(|_: Query<&Position>| {});

        schedule.build_plan();

        assert_eq!(schedule.plan.len(), 1);
        assert_eq!(schedule.plan[0].len(), 2);
    }

    #[test]
    fn plan_splits_conflicting_accesses_into_separate_batches() {
        let mut schedule = Schedule::default();
        schedule.add_system(|_: Query<&mut Position>| {});
        schedule.add_system(|_: Query<&Position>| {});

        schedule.build_plan();

        assert_eq!(schedule.plan.len(), 2);
        assert!(schedule.plan.iter().all(|batch| batch.len() == 1));
    }

    #[test]
    fn plan_is_deterministic_across_independent_builds() {
        let mut plans = Vec::new();
        for _ in 0..32 {
            let mut schedule = Schedule::default();
            let first = |_: ()| {};
            let second = |_: ()| {};
            let third = |_: ()| {};
            schedule.add_system(first);
            schedule.add_system(second.after(first));
            schedule.add_system(third.after(first));

            schedule.build_plan();
            plans.push(schedule.plan.clone());
        }

        // 每个 Schedule 的 HashMap 随机种子不同，没有排序时这里会出现分歧
        assert!(plans.windows(2).all(|pair| pair[0] == pair[1]));

        // 无依赖可同批的 system 按 SystemId（注册顺序）升序排列
        assert_eq!(
            plans[0],
            [vec![SystemId(0)], vec![SystemId(1), SystemId(2)]]
        );
    }

    #[test]
    fn plan_grouping_is_deterministic_across_independent_builds() {
        let mut plans = Vec::new();
        for _ in 0..32 {
            let mut schedule = Schedule::default();
            schedule.add_system(|_: Query<&mut Position>| {});
            schedule.add_system(|_: Query<&Position>| {});
            schedule.add_system(|_: ()| {});
            schedule.add_system(|_: ()| {});

            schedule.build_plan();
            plans.push(schedule.plan.clone());
        }

        // 初始 ready 里有多个候选时，遍历顺序决定谁和冲突方同批
        assert!(plans.windows(2).all(|pair| pair[0] == pair[1]));

        // 写入方先入批，两个自由 system 跟它同批；读取方被拆到第二批
        assert_eq!(
            plans[0],
            [
                vec![SystemId(0), SystemId(2), SystemId(3)],
                vec![SystemId(1)],
            ]
        );
    }

    #[test]
    fn plan_orders_systems_by_declared_dependencies() {
        let mut schedule = Schedule::default();
        let first = |_: ()| {};
        let second = |_: ()| {};
        let first_id = schedule.add_system(first);
        let second_id = schedule.add_system(second.after(first));

        schedule.build_plan();

        assert_eq!(schedule.plan.len(), 2);
        assert!(schedule.plan[0] == [first_id]);
        assert!(schedule.plan[1] == [second_id]);
    }

    /// 记录每次 `run` 收到几个批次、每批几个 system，并按批次顺序执行它们。
    #[derive(Default)]
    struct RecordingExecutor {
        runs: Arc<Mutex<Vec<Vec<usize>>>>,
    }

    impl SystemExecutor for RecordingExecutor {
        fn run(
            &mut self,
            plan: &mut SystemPlan<'_>,
            world: &mut World,
            _error_handler: ErrorHandler,
        ) {
            let cell = WorldCell::new(world);
            let mut sizes = Vec::with_capacity(plan.batch_count());
            for index in 0..plan.batch_count() {
                let systems = plan.batch_systems(index);
                sizes.push(systems.len());
                for system in systems {
                    system.run(&cell);
                }
            }
            self.runs.lock().unwrap().push(sizes);
        }
    }

    #[test]
    fn system_ids_are_dense_indices() {
        let mut schedule = Schedule::default();
        let first = schedule.add_system(|_: ()| {});
        let second = schedule.add_system(|_: ()| {});
        let third = schedule.add_system(|_: ()| {});

        assert!(first == SystemId(0));
        assert!(second == SystemId(1));
        assert!(third == SystemId(2));
    }

    #[test]
    fn plan_view_exposes_every_system_exactly_once() {
        let log = Arc::new(Mutex::new(Vec::new()));
        let executor = RecordingExecutor::default();
        let runs = executor.runs.clone();

        let mut schedule = Schedule::default();
        schedule.set_executor(executor);

        // 写入和读取同一组件的两个 system 必然分属不同批次，
        // 两个无访问的 system 总会并入第一个批次，于是某个批次里一定出现下标不连续。
        let write_log = log.clone();
        schedule.add_system(move |_: Query<&mut Position>| {
            write_log.lock().unwrap().push("write");
        });
        let read_log = log.clone();
        schedule.add_system(move |_: Query<&Position>| {
            read_log.lock().unwrap().push("read");
        });
        let first_free_log = log.clone();
        schedule.add_system(move |_: ()| first_free_log.lock().unwrap().push("free-a"));
        let second_free_log = log.clone();
        schedule.add_system(move |_: ()| second_free_log.lock().unwrap().push("free-b"));

        schedule.run(&mut World::new());

        let recorded = runs.lock().unwrap();

        // 执行器只被调用一次，拿到的是整份计划而不是单个批次
        assert_eq!(recorded.len(), 1);
        let sizes = &recorded[0];
        assert_eq!(sizes.len(), 2);

        let mut sorted = sizes.clone();
        sorted.sort_unstable();
        assert_eq!(sorted, [1, 3]);

        // 每个 system 恰好被交给执行器一次：没有重复、没有遗漏，且拿到的确实是它自己
        let mut labels = log.lock().unwrap().clone();
        labels.sort_unstable();
        assert_eq!(labels, ["free-a", "free-b", "read", "write"]);
    }

    #[test]
    fn executor_is_called_once_per_run_with_the_whole_plan() {
        let executor = RecordingExecutor::default();
        let runs = executor.runs.clone();

        let mut schedule = Schedule::default();
        schedule.set_executor(executor);
        schedule.add_system(|_: Query<&mut Position>| {});
        schedule.add_system(|_: Query<&Position>| {});

        schedule.run(&mut World::new());
        schedule.run(&mut World::new());

        {
            let recorded = runs.lock().unwrap();
            assert_eq!(recorded.len(), 2);
            // 计划被缓存，两次 run 交给执行器的批次结构一致
            assert_eq!(recorded[0], recorded[1]);
        }

        // 新增 system 让计划失效并重建，执行器下一次拿到的是新计划
        schedule.add_system(|_: ()| {});
        schedule.run(&mut World::new());

        let recorded = runs.lock().unwrap();
        assert_eq!(recorded.len(), 3);
        assert_eq!(recorded[2].len(), 2);
        assert_eq!(recorded[2].iter().sum::<usize>(), 3);
    }
}
