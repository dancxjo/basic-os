//! Kernel orchestration layer that embeds the graph and schedules bundles.
//!
//! This crate intentionally keeps hardware abstractions out of sight: the
//! `arch/` crates provide bring-up while this module focuses on graph-first
//! task orchestration.

use std::collections::{BTreeMap, VecDeque};

use thingos2_core::{
    BundleId, EdgeId, Graph, GraphError, NodeId, NodePattern, Value, WatchEvent, WatchId,
};

/// Unique identifier for kernel-managed tasks.
#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TaskId(pub u64);

/// Execution state for a task.
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum TaskState {
    Ready,
    Running,
    Sleeping,
    Blocked,
}

/// Kernel-visible task metadata.
#[derive(Debug)]
pub struct Task {
    pub id: TaskId,
    pub bundle: BundleId,
    pub state: TaskState,
    pub name: String,
}

/// Minimal scheduler model. Preemption is delegated to the architecture layer;
/// this struct tracks intent and task ordering.
#[derive(Debug, Default)]
pub struct Scheduler {
    next_task: u64,
    tasks: BTreeMap<TaskId, Task>,
    ready: VecDeque<TaskId>,
}

impl Scheduler {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn spawn(&mut self, bundle: BundleId, name: &str) -> TaskId {
        let id = TaskId(self.next_task);
        self.next_task += 1;
        let task = Task {
            id,
            bundle,
            state: TaskState::Ready,
            name: name.to_string(),
        };
        self.tasks.insert(id, task);
        self.ready.push_back(id);
        id
    }

    pub fn schedule_next(&mut self) -> Option<TaskId> {
        self.ready.pop_front().map(|task_id| {
            if let Some(task) = self.tasks.get_mut(&task_id) {
                task.state = TaskState::Running;
            }
            task_id
        })
    }

    pub fn yield_task(&mut self, task_id: TaskId) {
        if let Some(task) = self.tasks.get_mut(&task_id) {
            task.state = TaskState::Ready;
            self.ready.push_back(task_id);
        }
    }
}

/// Kernel runtime that exposes the syscall surface to bundles.
#[derive(Debug, Default)]
pub struct Kernel {
    graph: Graph,
    scheduler: Scheduler,
}

impl Kernel {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn graph(&self) -> &Graph {
        &self.graph
    }

    pub fn graph_mut(&mut self) -> &mut Graph {
        &mut self.graph
    }

    pub fn scheduler(&self) -> &Scheduler {
        &self.scheduler
    }

    pub fn scheduler_mut(&mut self) -> &mut Scheduler {
        &mut self.scheduler
    }

    /// Register a bundle in the graph and allocate a task for it.
    pub fn launch_bundle(&mut self, name: &str) -> (BundleId, TaskId) {
        let bundle = BundleId(self.scheduler.next_task + 1);
        self.graph.ensure_bundle_node(bundle, Some(name));
        let task = self.scheduler.spawn(bundle, name);
        (bundle, task)
    }

    /// Simplified syscall entry that bundles would invoke via trap/interrupt.
    pub fn syscall_graph_fiat_node(
        &mut self,
        bundle: BundleId,
        labels: impl IntoIterator<Item = String>,
        props: BTreeMap<String, Value>,
    ) -> Result<NodeId, GraphError> {
        self.graph.fiat_node(bundle, labels, props)
    }

    pub fn syscall_graph_link(
        &mut self,
        bundle: BundleId,
        kind: String,
        from: NodeId,
        to: NodeId,
        props: BTreeMap<String, Value>,
    ) -> Result<EdgeId, GraphError> {
        self.graph.link(bundle, kind, from, to, props)
    }

    pub fn syscall_graph_get_nodes(
        &self,
        bundle: BundleId,
        pattern: &NodePattern,
    ) -> Result<Vec<NodeId>, GraphError> {
        self.graph.get_nodes(bundle, pattern)
    }

    pub fn syscall_graph_get_props(
        &self,
        bundle: BundleId,
        node: NodeId,
        keys: &[String],
    ) -> Result<BTreeMap<String, Value>, GraphError> {
        self.graph.get_props(bundle, node, keys)
    }

    pub fn syscall_graph_set_props(
        &mut self,
        bundle: BundleId,
        node: NodeId,
        props: BTreeMap<String, Value>,
    ) -> Result<(), GraphError> {
        self.graph.set_props(bundle, node, props)
    }

    pub fn syscall_watch_register(
        &mut self,
        bundle: BundleId,
        pattern: NodePattern,
    ) -> Result<WatchId, GraphError> {
        self.graph.watch_register(bundle, pattern)
    }

    pub fn syscall_watch_poll(&mut self, bundle: BundleId) -> Vec<WatchEvent> {
        self.graph.watch_poll(bundle)
    }

    /// Privileged helper for granting capability edges between bundles.
    pub fn grant_capability_edge(
        &mut self,
        issuer: BundleId,
        grantee: BundleId,
        kind: &str,
        resource: NodeId,
        props: BTreeMap<String, Value>,
        link_target: Option<NodeId>,
    ) -> Result<EdgeId, GraphError> {
        self.graph
            .grant_capability_edge(issuer, grantee, kind, resource, props, link_target)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kernel_launches_and_schedules_bundles() {
        let mut kernel = Kernel::new();
        let (bundle, task) = kernel.launch_bundle("framebuffer");
        assert_eq!(bundle, BundleId(1));
        assert_eq!(task, TaskId(0));

        let next = kernel.scheduler_mut().schedule_next();
        assert_eq!(next, Some(task));
    }

    #[test]
    fn syscalls_delegate_to_graph() {
        let mut kernel = Kernel::new();
        let (bundle, _) = kernel.launch_bundle("keyboard");
        let node = kernel
            .syscall_graph_fiat_node(bundle, vec!["KeyboardEvent".to_string()], BTreeMap::new())
            .unwrap();
        let pattern = NodePattern::default().with_label("KeyboardEvent");
        let nodes = kernel
            .syscall_graph_get_nodes(bundle, &pattern)
            .expect("query nodes");
        assert_eq!(nodes, vec![node]);
    }
}
