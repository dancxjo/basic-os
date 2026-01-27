//! Core graph and capability model for ThingOS v2.
//!
//! The goal is to keep all state visible through a graph surface. Bundles
//! manipulate the graph via kernel-managed rules enforced here.

use std::collections::{BTreeMap, BTreeSet, VecDeque};

/// Identifier for a bundle/task.
#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct BundleId(pub u64);

/// Opaque identifier for nodes.
#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct NodeId(pub u64);

/// Opaque identifier for edges.
#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct EdgeId(pub u64);

/// Identifier for watches registered by bundles.
#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct WatchId(pub u64);

/// Property value used by the graph.
#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    Int(i64),
    Float(f64),
    Bool(bool),
    Text(String),
    U128(u128),
}

impl From<&str> for Value {
    fn from(value: &str) -> Self {
        Self::Text(value.to_string())
    }
}

/// A graph node with labels, properties, and an owner bundle.
#[derive(Debug, Clone)]
pub struct Node {
    pub id: NodeId,
    pub labels: BTreeSet<String>,
    pub props: BTreeMap<String, Value>,
    pub owner: BundleId,
}

impl Node {
    pub fn has_label(&self, label: &str) -> bool {
        self.labels.contains(label)
    }

    pub fn matches_pattern(&self, pattern: &NodePattern) -> bool {
        if !pattern.labels.is_subset(&self.labels) {
            return false;
        }

        pattern
            .required_props
            .iter()
            .all(|(k, v)| self.props.get(k) == Some(v))
    }
}

/// A graph edge.
#[derive(Debug, Clone)]
pub struct Edge {
    pub id: EdgeId,
    pub kind: String,
    pub from: NodeId,
    pub to: NodeId,
    pub props: BTreeMap<String, Value>,
    pub owner: BundleId,
}

/// Pattern for querying nodes.
#[derive(Debug, Clone, Default)]
pub struct NodePattern {
    pub labels: BTreeSet<String>,
    pub required_props: BTreeMap<String, Value>,
}

impl NodePattern {
    pub fn with_label(mut self, label: &str) -> Self {
        self.labels.insert(label.to_string());
        self
    }

    pub fn with_prop(mut self, key: &str, value: Value) -> Self {
        self.required_props.insert(key.to_string(), value);
        self
    }
}

/// Reasons a watch fired.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WatchReason {
    NodeCreated,
    PropertiesChanged,
    EdgeCreated(EdgeId),
}

/// Event delivered to bundles for matching watches.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WatchEvent {
    pub watch_id: WatchId,
    pub node_id: NodeId,
    pub reason: WatchReason,
}

/// Errors returned by graph operations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GraphError {
    NodeNotFound,
    EdgeNotFound,
    Forbidden,
    WatchNotFound,
}

/// In-memory graph implementation suitable for kernel integration.
#[derive(Debug, Default)]
pub struct Graph {
    next_node: u64,
    next_edge: u64,
    next_watch: u64,
    nodes: BTreeMap<NodeId, Node>,
    edges: BTreeMap<EdgeId, Edge>,
    bundle_nodes: BTreeMap<BundleId, NodeId>,
    watches: BTreeMap<WatchId, (BundleId, NodePattern)>,
    watch_events: BTreeMap<BundleId, VecDeque<WatchEvent>>,
}

impl Graph {
    /// Create an empty graph.
    pub fn new() -> Self {
        Self::default()
    }

    /// Create (or return) the node that represents the bundle itself.
    pub fn ensure_bundle_node(&mut self, bundle: BundleId, name: Option<&str>) -> NodeId {
        if let Some(id) = self.bundle_nodes.get(&bundle) {
            return *id;
        }

        let node_id = self.alloc_node_id();
        let mut labels = BTreeSet::new();
        labels.insert("Bundle".to_string());
        let mut props = BTreeMap::new();
        if let Some(name) = name {
            props.insert("name".to_string(), Value::from(name));
        }
        let node = Node {
            id: node_id,
            labels,
            props,
            owner: bundle,
        };
        self.nodes.insert(node_id, node);
        self.bundle_nodes.insert(bundle, node_id);
        node_id
    }

    /// Create a node on behalf of a bundle.
    pub fn fiat_node(
        &mut self,
        bundle: BundleId,
        labels: impl IntoIterator<Item = String>,
        props: BTreeMap<String, Value>,
    ) -> Result<NodeId, GraphError> {
        let node_id = self.alloc_node_id();
        let labels_set = labels.into_iter().collect();
        let node = Node {
            id: node_id,
            labels: labels_set,
            props,
            owner: bundle,
        };
        self.nodes.insert(node_id, node);

        let bundle_node = self.ensure_bundle_node(bundle, None);
        self.create_edge_internal(
            bundle,
            "owns".to_string(),
            bundle_node,
            node_id,
            BTreeMap::new(),
            true,
        )?;

        self.enqueue_watch_events(bundle, node_id, WatchReason::NodeCreated);
        Ok(node_id)
    }

    /// Create an edge after enforcing ownership and capability rules.
    pub fn link(
        &mut self,
        bundle: BundleId,
        kind: String,
        from: NodeId,
        to: NodeId,
        props: BTreeMap<String, Value>,
    ) -> Result<EdgeId, GraphError> {
        let owns_from = self.is_owned_by(bundle, from)?;
        let owns_to = self.is_owned_by(bundle, to)?;
        if !(owns_from || owns_to || self.can_link(bundle, &kind, from, to)?) {
            return Err(GraphError::Forbidden);
        }

        let edge_id = self.create_edge_internal(bundle, kind.clone(), from, to, props, false)?;
        self.enqueue_watch_events(bundle, from, WatchReason::EdgeCreated(edge_id));
        self.enqueue_watch_events(bundle, to, WatchReason::EdgeCreated(edge_id));
        Ok(edge_id)
    }

    /// Query nodes visible to the bundle that match a pattern.
    pub fn get_nodes(
        &self,
        bundle: BundleId,
        pattern: &NodePattern,
    ) -> Result<Vec<NodeId>, GraphError> {
        let visible: Vec<NodeId> = self
            .nodes
            .values()
            .filter(|node| self.can_read_node(bundle, node).unwrap_or(false))
            .filter(|node| node.matches_pattern(pattern))
            .map(|node| node.id)
            .collect();
        Ok(visible)
    }

    /// Retrieve a subset of properties if permitted.
    pub fn get_props(
        &self,
        bundle: BundleId,
        node_id: NodeId,
        keys: &[String],
    ) -> Result<BTreeMap<String, Value>, GraphError> {
        let node = self.nodes.get(&node_id).ok_or(GraphError::NodeNotFound)?;
        if !self.can_read_node(bundle, node)? {
            return Err(GraphError::Forbidden);
        }

        let mut out = BTreeMap::new();
        for key in keys {
            if let Some(value) = node.props.get(key) {
                out.insert(key.clone(), value.clone());
            }
        }
        Ok(out)
    }

    /// Set properties on a node if ownership or write capability allows it.
    pub fn set_props(
        &mut self,
        bundle: BundleId,
        node_id: NodeId,
        props: BTreeMap<String, Value>,
    ) -> Result<(), GraphError> {
        let owned = self.is_owned_by(bundle, node_id)?;
        let writable = self.can_write(bundle, node_id)?;
        if !(owned || writable) {
            return Err(GraphError::Forbidden);
        }

        let node = self
            .nodes
            .get_mut(&node_id)
            .ok_or(GraphError::NodeNotFound)?;
        for (k, v) in props {
            node.props.insert(k, v);
        }
        self.enqueue_watch_events(bundle, node_id, WatchReason::PropertiesChanged);
        Ok(())
    }

    /// Register a watch for a pattern.
    pub fn watch_register(
        &mut self,
        bundle: BundleId,
        pattern: NodePattern,
    ) -> Result<WatchId, GraphError> {
        let id = WatchId(self.next_watch);
        self.next_watch += 1;
        self.watches.insert(id, (bundle, pattern));
        self.watch_events.entry(bundle).or_default();
        Ok(id)
    }

    /// Retrieve queued watch events for a bundle.
    pub fn watch_poll(&mut self, bundle: BundleId) -> Vec<WatchEvent> {
        self.watch_events
            .entry(bundle)
            .or_default()
            .drain(..)
            .collect()
    }

    /// Grant a capability edge from one bundle to a resource on behalf of
    /// another bundle. The issuer is trusted (typically the kernel or a
    /// privileged supervisor bundle) and bypasses ownership checks.
    pub fn grant_capability_edge(
        &mut self,
        issuer: BundleId,
        grantee: BundleId,
        kind: &str,
        resource: NodeId,
        mut props: BTreeMap<String, Value>,
        link_target: Option<NodeId>,
    ) -> Result<EdgeId, GraphError> {
        if kind == "CAN_LINK" {
            let to = link_target.unwrap_or(resource);
            props
                .entry("target".to_string())
                .or_insert(Value::U128(to.0 as u128));
        }

        let from = self.ensure_bundle_node(grantee, None);
        self.create_edge_internal(issuer, kind.to_string(), from, resource, props, true)
    }

    fn alloc_node_id(&mut self) -> NodeId {
        let id = NodeId(self.next_node);
        self.next_node += 1;
        id
    }

    fn alloc_edge_id(&mut self) -> EdgeId {
        let id = EdgeId(self.next_edge);
        self.next_edge += 1;
        id
    }

    fn create_edge_internal(
        &mut self,
        bundle: BundleId,
        kind: String,
        from: NodeId,
        to: NodeId,
        props: BTreeMap<String, Value>,
        skip_checks: bool,
    ) -> Result<EdgeId, GraphError> {
        if !skip_checks
            && !(self.is_owned_by(bundle, from)?
                || self.is_owned_by(bundle, to)?
                || self.can_link(bundle, &kind, from, to)?)
        {
            return Err(GraphError::Forbidden);
        }

        let id = self.alloc_edge_id();
        let edge = Edge {
            id,
            kind,
            from,
            to,
            props,
            owner: bundle,
        };
        self.edges.insert(id, edge);
        Ok(id)
    }

    fn can_read_node(&self, bundle: BundleId, node: &Node) -> Result<bool, GraphError> {
        if node.owner == bundle {
            return Ok(true);
        }

        let Some(bundle_node) = self.bundle_node_id_opt(bundle) else {
            return Ok(false);
        };
        Ok(self
            .edges
            .values()
            .any(|edge| edge.kind == "CAN_READ" && edge.from == bundle_node && edge.to == node.id))
    }

    fn can_write(&self, bundle: BundleId, node_id: NodeId) -> Result<bool, GraphError> {
        let Some(bundle_node) = self.bundle_node_id_opt(bundle) else {
            return Ok(false);
        };
        Ok(self
            .edges
            .values()
            .any(|edge| edge.kind == "CAN_WRITE" && edge.from == bundle_node && edge.to == node_id))
    }

    fn can_link(
        &self,
        bundle: BundleId,
        kind: &str,
        from: NodeId,
        to: NodeId,
    ) -> Result<bool, GraphError> {
        let Some(bundle_node) = self.bundle_node_id_opt(bundle) else {
            return Ok(false);
        };
        Ok(self.edges.values().any(|edge| {
            edge.kind == "CAN_LINK"
                && edge.props.get("kind") == Some(&Value::from(kind))
                && edge.from == bundle_node
                && edge.to == from
                && edge.props.get("target") == Some(&Value::U128(to.0 as u128))
        }))
    }

    fn is_owned_by(&self, bundle: BundleId, node_id: NodeId) -> Result<bool, GraphError> {
        let node = self.nodes.get(&node_id).ok_or(GraphError::NodeNotFound)?;
        if node.owner == bundle {
            return Ok(true);
        }

        let Some(bundle_node) = self.bundle_node_id_opt(bundle) else {
            return Ok(false);
        };
        Ok(self
            .edges
            .values()
            .any(|edge| edge.kind == "owns" && edge.from == bundle_node && edge.to == node_id))
    }

    fn bundle_node_id_opt(&self, bundle: BundleId) -> Option<NodeId> {
        self.bundle_nodes.get(&bundle).copied()
    }

    fn enqueue_watch_events(&mut self, bundle: BundleId, node_id: NodeId, reason: WatchReason) {
        for (watch_id, (owner, pattern)) in self.watches.iter() {
            if *owner != bundle {
                // Watches obey visibility: skip if owner cannot read node.
                if let Some(node) = self.nodes.get(&node_id) {
                    if !self.can_read_node(*owner, node).unwrap_or(false) {
                        continue;
                    }
                }
            }
            if let Some(node) = self.nodes.get(&node_id) {
                if node.matches_pattern(pattern) {
                    self.watch_events
                        .entry(*owner)
                        .or_default()
                        .push_back(WatchEvent {
                            watch_id: *watch_id,
                            node_id,
                            reason: reason.clone(),
                        });
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn setup_graph() -> Graph {
        Graph::new()
    }

    #[test]
    fn bundle_owns_nodes_it_creates() {
        let mut graph = setup_graph();
        let bundle = BundleId(1);
        let node = graph
            .fiat_node(bundle, vec!["Window".to_string()], BTreeMap::new())
            .expect("node creation");
        assert!(graph.is_owned_by(bundle, node).unwrap());
    }

    #[test]
    fn forbids_cross_bundle_edge_creation_without_capability() {
        let mut graph = setup_graph();
        let a = BundleId(1);
        let b = BundleId(2);
        graph
            .fiat_node(a, vec!["Surface".to_string()], BTreeMap::new())
            .unwrap();
        let node_b1 = graph
            .fiat_node(b, vec!["Window".to_string()], BTreeMap::new())
            .unwrap();
        let node_b2 = graph
            .fiat_node(b, vec!["Cursor".to_string()], BTreeMap::new())
            .unwrap();

        let result = graph.link(
            a,
            "HAS_SURFACE".to_string(),
            node_b1,
            node_b2,
            BTreeMap::new(),
        );
        assert_eq!(result, Err(GraphError::Forbidden));
    }

    #[test]
    fn allow_edges_into_owned_nodes() {
        let mut graph = setup_graph();
        let owner = BundleId(1);
        let other = BundleId(2);
        let owned = graph
            .fiat_node(owner, vec!["Surface".to_string()], BTreeMap::new())
            .unwrap();
        let node_other = graph
            .fiat_node(other, vec!["Window".to_string()], BTreeMap::new())
            .unwrap();

        let result = graph.link(
            owner,
            "HAS_SURFACE".to_string(),
            node_other,
            owned,
            BTreeMap::new(),
        );
        assert_eq!(result, Ok(EdgeId(2))); // 0 + 1 for owns edges, 2 for new link
    }

    #[test]
    fn denies_property_writes_without_rights() {
        let mut graph = setup_graph();
        let a = BundleId(1);
        let b = BundleId(2);
        let node = graph
            .fiat_node(a, vec!["Window".to_string()], BTreeMap::new())
            .unwrap();

        let mut props = BTreeMap::new();
        props.insert("title".to_string(), Value::from("hi"));
        let res = graph.set_props(b, node, props);
        assert_eq!(res, Err(GraphError::Forbidden));
    }

    #[test]
    fn watches_fire_on_creation_and_updates() {
        let mut graph = setup_graph();
        let bundle = BundleId(1);
        let pattern = NodePattern::default().with_label("KeyboardEvent");
        let watch_id = graph.watch_register(bundle, pattern).unwrap();

        let props = BTreeMap::from([("key".to_string(), Value::from("A"))]);
        let node = graph
            .fiat_node(bundle, vec!["KeyboardEvent".to_string()], props)
            .unwrap();

        let mut events = graph.watch_poll(bundle);
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].watch_id, watch_id);
        assert_eq!(events[0].node_id, node);
        assert_eq!(events[0].reason, WatchReason::NodeCreated);

        let mut props = BTreeMap::new();
        props.insert("key".to_string(), Value::from("B"));
        graph.set_props(bundle, node, props).unwrap();
        events = graph.watch_poll(bundle);
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].reason, WatchReason::PropertiesChanged);
    }
}
