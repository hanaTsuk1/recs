use std::{
    any::{TypeId, type_name},
    collections::{HashMap, HashSet, VecDeque},
};

use crate::{
    World,
    access::Access,
    error::{RecsError, RecsResult},
    executor::{MultiThreadedExecutor, SystemExecutor},
    system::{FunctionSystem, IntoSystem, System, SystemParam, SystemParamFunction},
};

#[derive(Default)]
pub struct Schedule {
    graph: DependencyGraph,
}

impl Schedule {
    pub fn add_system<T: System>(&mut self, system_config: impl IntoSystemConfig<T>) -> SystemId {
        let system_config = system_config.into_config();
        self.graph.insert(system_config)
    }

    pub fn run(&mut self, world: &mut World) {
        let mut executor = MultiThreadedExecutor;
        executor.run(self, world);
    }

    pub fn build_plan(&mut self) -> Vec<Vec<SystemId>> {
        match self.build_plan_inner() {
            Ok(batches) => batches,
            Err(err) => panic!("failed to build schedule plan: {:?}", err),
        }
    }

    fn build_plan_inner(&mut self) -> RecsResult<Vec<Vec<SystemId>>> {
        self.graph.build_dependencies()?;
        let dependencies = self.graph.build_system_dependencies();
        self.graph.build_batches(&dependencies)
    }

    pub fn get_systems(&mut self, batch: &Vec<SystemId>) -> Vec<&mut Box<dyn System>> {
        self.graph.get_systems(batch)
    }
}

pub struct SystemConfig<T: System> {
    system: T,
    system_type_id: TypeId,
    before: Vec<SystemConfigOrderItem>,
    after: Vec<SystemConfigOrderItem>,
}

struct SystemConfigOrderItem {
    type_id: TypeId,
    type_name: &'static str,
}

impl<T: System> SystemConfig<T> {
    fn before<S: System, C: IntoSystemConfig<S>>(mut self, _config: C) -> SystemConfig<T> {
        self.before.push(SystemConfigOrderItem {
            type_id: TypeId::of::<C>(),
            type_name: type_name::<C>(),
        });
        self
    }

    fn after<S: System, C: IntoSystemConfig<S>>(mut self, _config: C) -> SystemConfig<T> {
        self.after.push(SystemConfigOrderItem {
            type_id: TypeId::of::<C>(),
            type_name: type_name::<C>(),
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
            before: vec![],
            after: vec![],
        }
    }
}

#[derive(Default)]
struct DependencyGraph {
    systems: HashMap<SystemId, Box<dyn System>>,
    system_types: HashMap<TypeId, SystemSetId>,
    system_sets: HashMap<SystemSetId, HashSet<SystemId>>,
    pending_dependencies: Vec<PendingDependency>,
    dependencies: HashMap<NodeId, HashSet<NodeId>>,
    next_id: u32,
    next_set_id: u32,
}

#[derive(PartialEq, Eq, Hash, Clone, Copy)]
pub struct SystemId(u32);

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
        let system_id = SystemId(self.next_id);
        self.next_id += 1;

        let type_id = config.system_type_id;
        self.systems.insert(system_id, Box::new(system));

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

        for system_id in self.systems.keys() {
            dependencies.entry(NodeId::System(*system_id)).or_default();
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

    fn build_system_dependencies(&self) -> HashMap<SystemId, HashSet<SystemId>> {
        let mut dependencies = HashMap::<SystemId, HashSet<SystemId>>::new();

        for system_id in self.systems.keys() {
            dependencies.entry(*system_id).or_default();
        }

        for (from, targets) in &self.dependencies {
            for to in targets {
                match (from, to) {
                    (NodeId::System(a), NodeId::System(b)) => {
                        dependencies.entry(*a).or_default().insert(*b);
                    }

                    (NodeId::System(a), NodeId::Set(set)) => {
                        for b in &self.system_sets[set] {
                            dependencies.entry(*a).or_default().insert(*b);
                        }
                    }

                    (NodeId::Set(set), NodeId::System(b)) => {
                        for a in &self.system_sets[set] {
                            dependencies.entry(*a).or_default().insert(*b);
                        }
                    }

                    (NodeId::Set(a), NodeId::Set(b)) => {
                        for from in &self.system_sets[a] {
                            for to in &self.system_sets[b] {
                                dependencies.entry(*from).or_default().insert(*to);
                            }
                        }
                    }
                }
            }
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

    fn build_batches(
        &self,
        dependencies: &HashMap<SystemId, HashSet<SystemId>>,
    ) -> RecsResult<Vec<Vec<SystemId>>> {
        let mut in_degrees = HashMap::<SystemId, usize>::new();

        // 初始化所有节点
        for system_id in self.systems.keys() {
            in_degrees.insert(*system_id, 0);
        }

        // 统计入度
        for targets in dependencies.values() {
            for target in targets {
                *in_degrees.entry(*target).or_default() += 1;
            }
        }

        // 初始 ready 集合
        let mut ready = in_degrees
            .iter()
            .filter_map(|(id, degree)| (*degree == 0).then_some(*id))
            .collect::<VecDeque<_>>();

        let mut batches = Vec::new();
        let mut processed = 0;

        while !ready.is_empty() {
            let mut batch = Vec::new();
            let mut deferred = VecDeque::new();

            // 从当前 ready 中贪心选择兼容的 System
            while let Some(candidate) = ready.pop_front() {
                let compatible = batch
                    .iter()
                    .all(|existing| self.can_run_together(candidate, *existing));

                if compatible {
                    batch.push(candidate);
                } else {
                    deferred.push_back(candidate);
                }
            }

            // 当前 Batch 已确定
            processed += batch.len();

            // 移除当前 Batch 的出边
            for system_id in &batch {
                if let Some(targets) = dependencies.get(system_id) {
                    for target in targets {
                        let degree = in_degrees
                            .get_mut(target)
                            .expect("dependency target must be a registered system");
                        *degree -= 1;

                        if *degree == 0 {
                            deferred.push_back(*target);
                        }
                    }
                }
            }

            batches.push(batch);
            ready = deferred;
        }

        // 环检测
        if processed != self.systems.len() {
            return Err(RecsError::CircularDependency);
        }

        Ok(batches)
    }

    fn can_run_together(&self, a: SystemId, b: SystemId) -> bool {
        let a = self
            .systems
            .get(&a)
            .expect("system id must belong to this dependency graph");
        let b = self
            .systems
            .get(&b)
            .expect("system id must belong to this dependency graph");
        let mut access = Access::default();
        a.init_access(&mut access);
        b.init_access(&mut access);
        access.validate().is_ok()
    }

    fn get_systems(&mut self, system_ids: &Vec<SystemId>) -> Vec<&mut Box<dyn System>> {
        self.systems
            .iter_mut()
            .filter_map(|(system_id, system)| system_ids.contains(system_id).then_some(system))
            .collect()
    }
}
