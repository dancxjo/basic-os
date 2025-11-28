use crate::graph::canon;
use crate::graph::canon::Symbol;
use crate::graph::events::{emit_edge_event, emit_thing_event, reflect_thing_side_effects};
use crate::graph::journal::Value;
use crate::graph::types::*;
use alloc::collections::{BTreeMap, BTreeSet};
use alloc::vec::Vec;
use uuid::Uuid;

const MAX_WATCH_QUEUE: usize = 1024;

#[derive(Default)]
pub struct Store {
    next_revision: u64,
    things: BTreeMap<Uuid, Vec<GraphThing>>,
    edges: Vec<GraphEdge>,
    kind_index: BTreeMap<Symbol, BTreeSet<Uuid>>,
    edges_by_src_pred: BTreeMap<(Uuid, Symbol), Vec<GraphEdge>>,
    watches: BTreeMap<WatchId, Watch>,
    next_watch_id: WatchId,
}

impl Store {
    pub fn new() -> Self {
        Self {
            next_revision: 1,
            things: BTreeMap::new(),
            edges: Vec::new(),
            kind_index: BTreeMap::new(),
            edges_by_src_pred: BTreeMap::new(),
            watches: BTreeMap::new(),
            next_watch_id: 1,
        }
    }

    pub fn register_watch(&mut self, owner: BundleId, query: WatchQuery) -> WatchId {
        let mut pattern = NodePattern::default();
        if let Some(kind) = query.kind {
            pattern.labels.push(kind);
        }
        if let Some(src) = query.src {
            pattern.props.insert(canon::SRC, Value::Uuid(src));
        }
        if let Some(dst) = query.dst {
            pattern.props.insert(canon::DST, Value::Uuid(dst));
        }
        self.register_watch_pattern(owner, pattern)
    }

    pub fn register_watch_pattern(&mut self, owner: BundleId, pattern: NodePattern) -> WatchId {
        let id = self.next_watch_id;
        self.next_watch_id += 1;
        self.watches.insert(
            id,
            Watch {
                id,
                owner,
                pattern,
                queue: Vec::new(),
            },
        );
        id
    }

    pub fn find_by_kind(&self, owner: BundleId, kind: &str, cursor: u64) -> (Vec<GraphThing>, u64) {
        let symbol = if let Some(s) = canon::from_str(kind) {
            s
        } else {
            return (Vec::new(), 0);
        };

        let mut results = Vec::new();
        let mut next_cursor = 0;
        let max_results = 100; // Limit results per call

        if let Some(uuids) = self.kind_index.get(&symbol) {
            let mut count = 0;
            let mut skipped = 0;

            // Simple cursor implementation: skip 'cursor' items
            // This is O(N) scan which is fine for now as per requirements
            for uuid in uuids {
                if (skipped as u64) < cursor {
                    skipped += 1;
                    continue;
                }

                if count >= max_results {
                    next_cursor = cursor + count as u64;
                    break;
                }

                if let Some(things) = self.things.get(uuid) {
                    if let Some(thing) = things.last() {
                        if !self.can_read(owner, thing.id) {
                            continue;
                        }
                        results.push(thing.clone());
                        count += 1;
                    }
                }
            }
        }

        (results, next_cursor)
    }

    pub fn poll_watch(&mut self, id: WatchId) -> Option<GraphWatchBatch> {
        if let Some(watch) = self.watches.get_mut(&id) {
            let events = watch.queue.clone();
            watch.queue.clear();
            Some(GraphWatchBatch {
                from_revision: events.first().map_or(0, |e| e.revision()),
                latest_revision: events.last().map_or(0, |e| e.revision()),
                changes: events,
            })
        } else {
            None
        }
    }

    pub fn fiat(&mut self, owner: BundleId, request: GraphFiatRequest) -> GraphThing {
        let mut labels = BTreeSet::new();
        labels.insert(request.kind);
        let node_request = GraphNodeRequest {
            id: request.id,
            labels: labels.iter().copied().collect(),
            props: request.fields,
        };
        self.fiat_node(owner, node_request)
    }

    pub fn fiat_node(&mut self, owner: BundleId, request: GraphNodeRequest) -> GraphThing {
        self.ensure_bundle_node(owner);
        let mut labels: BTreeSet<Symbol> = request.labels.iter().copied().collect();
        if labels.is_empty() {
            labels.insert(canon::THING_CREATED);
        }
        let kind = *labels.iter().next().unwrap_or(&canon::THING_CREATED);
        let mut props = request.props;
        props.entry(canon::OWNER).or_insert(Value::Uuid(owner));
        let id = request.id.unwrap_or_else(|| derive_uuid(kind, &props));
        let revision = self.next_revision();
        let thing = GraphThing {
            id,
            kind,
            labels,
            fields: props,
            owner,
            revision,
        };

        self.insert_thing(thing.clone());
        self.add_ownership_edge(owner, thing.id, revision);
        self.notify_watches(&GraphChange::Thing(thing.clone()));
        emit_thing_event(&thing);
        reflect_thing_side_effects(&thing);
        thing
    }

    pub fn that(&mut self, owner: BundleId, request: GraphThatRequest) -> u64 {
        let link_request = GraphLinkRequest {
            id: None,
            kind: request.pred,
            from: request.src,
            to: request.dst,
            props: request.props,
        };
        self.link_edge(owner, link_request)
    }

    pub fn link_edge(&mut self, owner: BundleId, request: GraphLinkRequest) -> u64 {
        if !self.can_link(owner, request.from, request.to, request.kind) {
            return 0;
        }
        let revision = self.next_revision();
        let edge = GraphEdge {
            id: request
                .id
                .unwrap_or_else(|| derive_uuid(request.kind, &request.props)),
            src: request.from,
            pred: request.kind,
            dst: request.to,
            props: request.props,
            owner,
            revision,
        };
        self.insert_edge(edge.clone());
        self.notify_watches(&GraphChange::Edge(edge.clone()));
        emit_edge_event(&edge);
        revision
    }

    pub fn get_nodes(&self, owner: BundleId, pattern: NodePattern) -> Vec<GraphThing> {
        let mut matches = Vec::new();
        for thing in self.things.values().filter_map(|v| v.last()) {
            if !self.can_read(owner, thing.id) {
                continue;
            }
            if Self::matches_pattern(&pattern, thing, None) {
                matches.push(thing.clone());
            }
        }
        matches
    }

    pub fn get_props(
        &self,
        owner: BundleId,
        request: GraphPropsGetRequest,
    ) -> Option<BTreeMap<Symbol, Value>> {
        let thing = self.latest(&request.node)?;
        if !self.can_read(owner, thing.id) {
            return None;
        }
        let mut out = BTreeMap::new();
        if request.keys.is_empty() {
            out.extend(thing.fields.iter().map(|(k, v)| (*k, v.clone())));
        } else {
            for key in request.keys.iter() {
                if let Some(val) = thing.fields.get(key) {
                    out.insert(*key, val.clone());
                }
            }
        }
        Some(out)
    }

    pub fn set_props(&mut self, owner: BundleId, request: GraphPropsRequest) -> bool {
        let Some(mut current) = self.latest(&request.node) else {
            return false;
        };
        if !self.can_write(owner, current.id) {
            return false;
        }
        for (k, v) in request.props.iter() {
            current.fields.insert(*k, v.clone());
        }
        current.revision = self.next_revision();
        self.insert_thing(current.clone());
        self.notify_watches(&GraphChange::Thing(current));
        true
    }

    /// Grant a capability to another bundle.
    /// The granting bundle must either own the target or have the capability itself.
    /// Returns true if the capability was successfully granted.
    pub fn grant_capability(&mut self, grantor: BundleId, request: GrantCapabilityRequest) -> bool {
        // Validate the capability symbol
        if !Self::is_data_capability(request.capability)
            && !Self::is_hardware_capability(request.capability)
        {
            return false;
        }

        // Enforce capability delegation policy:
        // - Hardware capabilities can only be minted by the kernel.
        // - Data capabilities can be granted by the owner of the target Thing
        //   (or by the kernel, which owns everything).
        let can_grant = if Self::is_hardware_capability(request.capability) {
            grantor == KERNEL_BUNDLE_ID
        } else {
            grantor == KERNEL_BUNDLE_ID || self.owns(grantor, request.target)
        };

        if !can_grant {
            return false;
        }

        // Ensure the grantee bundle exists as a node
        self.ensure_bundle_node(request.grantee);

        // Create the capability edge from grantee bundle to target
        let grantee_node = Self::bundle_node_id(request.grantee);
        let revision = self.next_revision();
        let mut props = BTreeMap::new();
        props.insert(canon::OWNER, Value::Uuid(grantor));
        props.insert(canon::DST, Value::Uuid(request.target));

        let edge = GraphEdge {
            id: derive_uuid(request.capability, &props),
            src: grantee_node,
            pred: request.capability,
            dst: request.target,
            props,
            owner: grantor,
            revision,
        };

        self.insert_edge(edge.clone());
        self.notify_watches(&GraphChange::Edge(edge.clone()));
        emit_edge_event(&edge);
        true
    }

    pub fn latest(&self, id: &Uuid) -> Option<GraphThing> {
        self.things
            .get(id)
            .and_then(|versions| versions.last())
            .cloned()
    }

    pub fn latest_of_kind(&self, kind: Symbol) -> Vec<GraphThing> {
        let Some(ids) = self.kind_index.get(&kind) else {
            return Vec::new();
        };
        ids.iter()
            .filter_map(|id| self.latest(id))
            .collect::<Vec<_>>()
    }

    pub fn edges_of(&self, src: Uuid, pred: Symbol) -> Vec<GraphEdge> {
        self.edges_by_src_pred
            .get(&(src, pred))
            .cloned()
            .unwrap_or_else(Vec::new)
    }

    /// Stub implementation returning an empty change batch with the latest revision.
    pub fn changes_since(&self, revision: u64) -> GraphWatchBatch {
        GraphWatchBatch {
            from_revision: revision,
            latest_revision: self.next_revision.saturating_sub(1),
            changes: Vec::new(),
        }
    }

    pub fn snapshot(&self) -> GraphSnapshot {
        let mut things = Vec::new();
        for versions in self.things.values() {
            things.extend(versions.iter().cloned());
        }

        GraphSnapshot {
            revision: self.next_revision.saturating_sub(1),
            thing_count: self.kind_index.values().map(BTreeSet::len).sum(),
            edge_count: self.edges.len(),
            things,
            edges: self.edges.clone(),
        }
    }

    pub fn apply_snapshot(&mut self, snapshot: GraphSnapshot) {
        self.things.clear();
        self.edges.clear();
        self.kind_index.clear();
        self.edges_by_src_pred.clear();
        self.watches.clear();

        for thing in snapshot.things.into_iter() {
            self.insert_thing(thing);
        }

        for edge in snapshot.edges.into_iter() {
            self.insert_edge(edge);
        }

        self.next_revision = snapshot.revision.saturating_add(1);
    }

    fn next_revision(&mut self) -> u64 {
        let rev = self.next_revision;
        self.next_revision = self.next_revision.wrapping_add(1);
        rev
    }

    fn notify_watches(&mut self, change: &GraphChange) {
        let (target, edge) = match change {
            GraphChange::Thing(t) => (Some(t.clone()), None),
            GraphChange::Edge(e) => (self.latest(&e.src).or_else(|| self.latest(&e.dst)), Some(e)),
        };

        let watch_specs: Vec<(WatchId, BundleId, NodePattern)> = self
            .watches
            .values()
            .map(|w| (w.id, w.owner, w.pattern.clone()))
            .collect();

        for (watch_id, owner, pattern) in watch_specs {
            if !self.change_visible_to(owner, change) {
                continue;
            }
            if let Some(node) = target.clone() {
                if Self::matches_pattern(&pattern, &node, edge) {
                    if let Some(watch) = self.watches.get_mut(&watch_id) {
                        watch.queue.push(change.clone());
                        if watch.queue.len() > MAX_WATCH_QUEUE {
                            // Drop oldest
                            watch.queue.remove(0);
                        }
                    }
                }
            }
        }
    }

    fn matches_pattern(
        pattern: &NodePattern,
        thing: &GraphThing,
        edge: Option<&GraphEdge>,
    ) -> bool {
        for label in pattern.labels.iter() {
            if !thing.labels.contains(label) {
                return false;
            }
        }
        for (k, v) in pattern.props.iter() {
            match (thing.fields.get(k), edge) {
                (Some(existing), _) if existing == v => {}
                (_, Some(e)) if *k == canon::SRC && *v == Value::Uuid(e.src) => {}
                (_, Some(e)) if *k == canon::DST && *v == Value::Uuid(e.dst) => {}
                (_, Some(e)) if *k == canon::PREDICATE && *v == Value::Symbol(e.pred) => {}
                _ => return false,
            }
        }
        true
    }

    fn bundle_node_id(bundle: BundleId) -> Uuid {
        bundle
    }

    fn ensure_bundle_node(&mut self, bundle: BundleId) {
        let id = Self::bundle_node_id(bundle);
        if self.things.get(&id).is_some() {
            return;
        }
        let mut labels = BTreeSet::new();
        labels.insert(canon::BUNDLE);
        let mut fields = BTreeMap::new();
        fields.insert(canon::ID, Value::Uuid(bundle));
        let thing = GraphThing {
            id,
            kind: canon::BUNDLE,
            labels,
            fields,
            owner: bundle,
            revision: self.next_revision(),
        };
        self.insert_thing(thing);
    }

    fn add_ownership_edge(&mut self, owner: BundleId, node: Uuid, revision: u64) {
        let mut props = BTreeMap::new();
        props.insert(canon::OWNER, Value::Uuid(owner));
        props.insert(canon::DST, Value::Uuid(node));
        let edge = GraphEdge {
            id: derive_uuid(canon::OWNS, &props),
            src: Self::bundle_node_id(owner),
            pred: canon::OWNS,
            dst: node,
            props,
            owner,
            revision,
        };
        self.insert_edge(edge);
    }

    fn owns(&self, bundle: BundleId, node: Uuid) -> bool {
        let bundle_node = Self::bundle_node_id(bundle);
        self.edges_by_src_pred
            .get(&(bundle_node, canon::OWNS))
            .map(|edges| edges.iter().any(|e| e.dst == node))
            .unwrap_or(false)
    }

    fn is_data_capability(capability: Symbol) -> bool {
        matches!(
            capability,
            canon::CAN_READ | canon::CAN_WRITE | canon::CAN_LINK
        )
    }

    fn is_hardware_capability(capability: Symbol) -> bool {
        matches!(
            capability,
            canon::CAN_HANDLE_IRQ | canon::CAN_DMA | canon::CAN_MMIO | canon::CAN_PORT_IO
        )
    }

    fn has_capability(&self, bundle: BundleId, target: Uuid, predicate: Symbol) -> bool {
        let bundle_node = Self::bundle_node_id(bundle);
        self.edges_by_src_pred
            .get(&(bundle_node, predicate))
            .map(|edges| edges.iter().any(|e| e.dst == target))
            .unwrap_or(false)
    }

    pub fn bundle_has_capability(
        &self,
        bundle: BundleId,
        target: Uuid,
        capability: Symbol,
    ) -> bool {
        self.has_capability(bundle, target, capability)
    }

    pub fn can_read(&self, bundle: BundleId, node: Uuid) -> bool {
        bundle == KERNEL_BUNDLE_ID
            || self.owns(bundle, node)
            || self.has_capability(bundle, node, canon::CAN_READ)
            || self.can_write(bundle, node)
    }

    fn can_write(&self, bundle: BundleId, node: Uuid) -> bool {
        bundle == KERNEL_BUNDLE_ID
            || self.owns(bundle, node)
            || self.has_capability(bundle, node, canon::CAN_WRITE)
    }

    fn can_link(&self, bundle: BundleId, from: Uuid, to: Uuid, kind: Symbol) -> bool {
        if bundle == KERNEL_BUNDLE_ID || self.owns(bundle, from) || self.owns(bundle, to) {
            return true;
        }
        let bundle_node = Self::bundle_node_id(bundle);
        if let Some(edges) = self.edges_by_src_pred.get(&(bundle_node, canon::CAN_LINK)) {
            for edge in edges.iter() {
                if edge.dst == from || edge.dst == to {
                    if let Some(Value::Symbol(cap_kind)) = edge.props.get(&canon::KIND) {
                        if *cap_kind == kind {
                            return true;
                        }
                    } else {
                        return true;
                    }
                }
            }
        }
        false
    }

    fn change_visible_to(&self, bundle: BundleId, change: &GraphChange) -> bool {
        match change {
            GraphChange::Thing(t) => self.can_read(bundle, t.id),
            GraphChange::Edge(e) => {
                self.can_read(bundle, e.src) || self.can_read(bundle, e.dst) || e.owner == bundle
            }
        }
    }

    fn insert_thing(&mut self, thing: GraphThing) {
        let versions = self.things.entry(thing.id).or_default();
        let should_add = versions
            .last()
            .map(|prev| thing.revision > prev.revision)
            .unwrap_or(true);
        if should_add {
            versions.push(thing.clone());
            self.kind_index
                .entry(thing.kind)
                .or_default()
                .insert(thing.id);
        }
    }

    fn insert_edge(&mut self, edge: GraphEdge) {
        let should_add = self
            .edges
            .last()
            .map(|prev| edge.revision > prev.revision)
            .unwrap_or(true);
        if should_add {
            self.edges.push(edge.clone());
            self.edges_by_src_pred
                .entry((edge.src, edge.pred))
                .or_default()
                .push(edge);
        }
    }
}

fn derive_uuid(kind: Symbol, fields: &BTreeMap<Symbol, Value>) -> Uuid {
    let mut name: Vec<u8> = Vec::new();
    name.extend_from_slice(&kind.0.to_be_bytes());
    if let Ok(buf) = postcard::to_allocvec(fields) {
        name.extend_from_slice(&buf);
    }
    Uuid::new_v5(&Uuid::NAMESPACE_OID, &name)
}
