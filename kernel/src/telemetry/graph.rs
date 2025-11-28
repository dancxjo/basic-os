use crate::telemetry::canon;
use crate::telemetry::canon::Symbol;
use crate::telemetry::journal::{self, Value};
use alloc::collections::{BTreeMap, BTreeSet};
use alloc::vec::Vec;
use serde::{Deserialize, Serialize};
use spin::Mutex;
use uuid::Uuid;

/// Identifier representing a bundle/authority.
pub type BundleId = Uuid;

/// Stable identifier for the kernel bundle. This bundle implicitly holds
/// all privileges and is used for early boot declarations.
pub const KERNEL_BUNDLE_ID: BundleId = Uuid::from_u128(0xfeed_cafe_dead_beef_cafe_babe_0000_0001);

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum GraphChange {
    Thing(GraphThing),
    Edge(GraphEdge),
}

impl GraphChange {
    pub fn revision(&self) -> u64 {
        match self {
            GraphChange::Thing(t) => t.revision,
            GraphChange::Edge(e) => e.revision,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphThing {
    pub id: Uuid,
    pub kind: Symbol,
    pub labels: BTreeSet<Symbol>,
    pub fields: BTreeMap<Symbol, Value>,
    pub owner: BundleId,
    pub revision: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphEdge {
    pub id: Uuid,
    pub src: Uuid,
    pub pred: Symbol,
    pub dst: Uuid,
    pub props: BTreeMap<Symbol, Value>,
    pub owner: BundleId,
    pub revision: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphSnapshot {
    pub revision: u64,
    pub thing_count: usize,
    pub edge_count: usize,
    pub things: Vec<GraphThing>,
    pub edges: Vec<GraphEdge>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphWatchBatch {
    pub from_revision: u64,
    pub latest_revision: u64,
    pub changes: Vec<GraphChange>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphFiatRequest {
    pub id: Option<Uuid>,
    pub kind: Symbol,
    pub fields: BTreeMap<Symbol, Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphThatRequest {
    pub src: Uuid,
    pub pred: Symbol,
    pub dst: Uuid,
    pub revision_hint: u64,
    #[serde(default)]
    pub props: BTreeMap<Symbol, Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WatchQuery {
    pub kind: Option<Symbol>,
    pub src: Option<Uuid>,
    pub dst: Option<Uuid>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct NodePattern {
    pub labels: Vec<Symbol>,
    #[serde(default)]
    pub props: BTreeMap<Symbol, Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphNodeRequest {
    pub id: Option<Uuid>,
    pub labels: Vec<Symbol>,
    pub props: BTreeMap<Symbol, Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphLinkRequest {
    pub id: Option<Uuid>,
    pub kind: Symbol,
    pub from: Uuid,
    pub to: Uuid,
    #[serde(default)]
    pub props: BTreeMap<Symbol, Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphPropsRequest {
    pub node: Uuid,
    pub props: BTreeMap<Symbol, Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphPropsGetRequest {
    pub node: Uuid,
    pub keys: Vec<Symbol>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SharedBufferSpec {
    pub id: Option<Uuid>,
    pub size_bytes: u64,
    pub kind: Symbol,
    pub usage: Symbol,
    #[serde(default)]
    pub addr: Option<u64>,
    #[serde(default)]
    pub props: BTreeMap<Symbol, Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueueStateSpec {
    pub id: Option<Uuid>,
    pub buffer: Uuid,
    pub owner: BundleId,
    pub head: u64,
    pub tail: u64,
    pub has_data: bool,
    #[serde(default)]
    pub capacity: Option<u64>,
    #[serde(default)]
    pub props: BTreeMap<Symbol, Value>,
}

/// Request to grant a capability from one bundle to another.
/// Data capabilities (CAN_READ/CAN_WRITE/CAN_LINK) may be delegated by the
/// owner of the target Thing. Hardware capabilities (IRQ, DMA, MMIO, PORT IO)
/// are kernel-only.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GrantCapabilityRequest {
    /// The bundle receiving the capability
    pub grantee: BundleId,
    /// The target node the capability applies to
    pub target: Uuid,
    /// The capability being granted (e.g., CAN_READ, CAN_WRITE, CAN_LINK)
    pub capability: Symbol,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[repr(C)]
pub struct GraphFindByKind {
    pub kind_ptr: u64, // *const u8
    pub kind_len: u64, // usize
    pub cursor: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[repr(C)]
pub struct GraphFindResultHeader {
    pub next_cursor: u64,
    pub count: u32,
}

pub type WatchId = u64;

pub struct Watch {
    id: WatchId,
    owner: BundleId,
    pattern: NodePattern,
    queue: Vec<GraphChange>,
}

const MAX_CHANGE_LOG: usize = 1024;
const MAX_WATCH_QUEUE: usize = 1024;

#[derive(Default)]
pub struct Store {
    next_revision: u64,
    things: BTreeMap<Uuid, Vec<GraphThing>>,
    edges: Vec<GraphEdge>,
    kind_index: BTreeMap<Symbol, BTreeSet<Uuid>>,
    edges_by_src_pred: BTreeMap<(Uuid, Symbol), Vec<GraphEdge>>,
    // changes: Vec<GraphChange>, // Removed global firehose
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
            // changes: Vec::new(),
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

    pub fn changes_since(&self, revision: u64) -> GraphWatchBatch {
        // Deprecated / Stubbed
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
        // self.changes.clear();
        self.watches.clear(); // Clear watches on snapshot apply? Or keep them?
        // Probably clear since state is reset.

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

    /*
    fn record_change(&mut self, change: GraphChange) {
        self.changes.push(change);
        if self.changes.len() > MAX_CHANGE_LOG {
            let overflow = self.changes.len().saturating_sub(MAX_CHANGE_LOG);
            self.changes.drain(0..overflow);
        }
    }
    */

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

    fn can_read(&self, bundle: BundleId, node: Uuid) -> bool {
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

static STORE: Mutex<Option<Store>> = Mutex::new(None);

pub fn init() {
    let mut s = STORE.lock();
    if s.is_none() {
        *s = Some(Store::new());
    }
}

pub fn with_store<R>(f: impl FnOnce(&mut Store) -> R) -> R {
    let mut s = STORE.lock();
    let store = s.get_or_insert_with(Store::new);
    f(store)
}

pub fn bundle_has_capability(bundle: BundleId, target: Uuid, capability: Symbol) -> bool {
    with_store(|store| store.bundle_has_capability(bundle, target, capability))
}

pub fn fiat(request: GraphFiatRequest) -> GraphThing {
    fiat_for_bundle(KERNEL_BUNDLE_ID, request)
}

pub fn fiat_for_bundle(owner: BundleId, request: GraphFiatRequest) -> GraphThing {
    with_store(|store| store.fiat(owner, request))
}

pub fn fiat_node(owner: BundleId, request: GraphNodeRequest) -> GraphThing {
    with_store(|store| store.fiat_node(owner, request))
}

pub fn that(request: GraphThatRequest) -> u64 {
    that_for_bundle(KERNEL_BUNDLE_ID, request)
}

pub fn that_for_bundle(owner: BundleId, request: GraphThatRequest) -> u64 {
    with_store(|store| store.that(owner, request))
}

pub fn link(owner: BundleId, request: GraphLinkRequest) -> u64 {
    with_store(|store| store.link_edge(owner, request))
}

pub fn get_nodes(owner: BundleId, pattern: NodePattern) -> Vec<GraphThing> {
    with_store(|store| store.get_nodes(owner, pattern))
}

pub fn get_props(
    owner: BundleId,
    request: GraphPropsGetRequest,
) -> Option<BTreeMap<Symbol, Value>> {
    with_store(|store| store.get_props(owner, request))
}

pub fn set_props(owner: BundleId, request: GraphPropsRequest) -> bool {
    with_store(|store| store.set_props(owner, request))
}

pub fn declare_shared_buffer(owner: BundleId, mut spec: SharedBufferSpec) -> GraphThing {
    let mut fields = spec.props;
    fields.insert(canon::BYTES, Value::U64(spec.size_bytes));
    fields.insert(canon::BUFFER_KIND, Value::Symbol(spec.kind));
    fields.insert(canon::BUFFER_USAGE, Value::Symbol(spec.usage));
    fields.insert(canon::OWNER, Value::Uuid(owner));
    if let Some(addr) = spec.addr {
        fields.insert(canon::ADDR, Value::U64(addr));
    }

    let request = GraphNodeRequest {
        id: spec.id,
        labels: vec![canon::SHARED_BUFFER],
        props: fields,
    };
    fiat_node(owner, request)
}

pub fn declare_queue_state(mut spec: QueueStateSpec) -> GraphThing {
    let mut fields = spec.props;
    fields.insert(canon::BUFFER, Value::Uuid(spec.buffer));
    fields.insert(canon::HEAD, Value::U64(spec.head));
    fields.insert(canon::TAIL, Value::U64(spec.tail));
    fields.insert(canon::HAS_DATA, Value::Bool(spec.has_data));
    fields.insert(canon::OWNER, Value::Uuid(spec.owner));
    if let Some(cap) = spec.capacity {
        fields.insert(canon::CAPACITY, Value::U64(cap));
    }

    let request = GraphNodeRequest {
        id: spec.id,
        labels: vec![canon::QUEUE_STATE],
        props: fields,
    };
    fiat_node(spec.owner, request)
}

pub fn update_queue_state(
    owner: BundleId,
    queue_id: Uuid,
    head: u64,
    tail: u64,
    has_data: bool,
    capacity: Option<u64>,
) -> bool {
    let mut props = BTreeMap::new();
    props.insert(canon::HEAD, Value::U64(head));
    props.insert(canon::TAIL, Value::U64(tail));
    props.insert(canon::HAS_DATA, Value::Bool(has_data));
    if let Some(cap) = capacity {
        props.insert(canon::CAPACITY, Value::U64(cap));
    }

    set_props(
        owner,
        GraphPropsRequest {
            node: queue_id,
            props,
        },
    )
}

pub fn grant_capability(grantor: BundleId, request: GrantCapabilityRequest) -> bool {
    with_store(|store| store.grant_capability(grantor, request))
}

pub fn get_thing(id: &Uuid) -> Option<GraphThing> {
    with_store(|store| store.latest(id))
}

pub fn get_things_of_kind(kind: Symbol) -> Vec<GraphThing> {
    with_store(|store| store.latest_of_kind(kind))
}

pub fn snapshot() -> GraphSnapshot {
    with_store(|store| store.snapshot())
}

pub fn apply_snapshot(snapshot: GraphSnapshot) {
    with_store(|store| store.apply_snapshot(snapshot));
}

pub fn export_snapshot_bytes() -> Option<Vec<u8>> {
    let snapshot = snapshot();
    postcard::to_allocvec(&snapshot).ok()
}

pub fn import_snapshot_bytes(buf: &[u8]) -> Result<(), postcard::Error> {
    let snapshot: GraphSnapshot = postcard::from_bytes(buf)?;
    apply_snapshot(snapshot);
    Ok(())
}

pub fn export_changes_since(revision: u64) -> Option<Vec<u8>> {
    let batch = with_store(|store| store.changes_since(revision));
    postcard::to_allocvec(&batch).ok()
}

pub fn export_thing_bytes(id: Uuid) -> Option<Vec<u8>> {
    let thing = get_thing(&id)?;
    postcard::to_allocvec(&thing).ok()
}

pub fn register_watch(owner: BundleId, query: WatchQuery) -> WatchId {
    with_store(|store| store.register_watch(owner, query))
}

pub fn register_watch_pattern(owner: BundleId, pattern: NodePattern) -> WatchId {
    with_store(|store| store.register_watch_pattern(owner, pattern))
}

pub fn export_find_by_kind_bytes(owner: BundleId, kind: &str, cursor: u64) -> Option<Vec<u8>> {
    let (things, next_cursor) = with_store(|store| store.find_by_kind(owner, kind, cursor));

    let header = GraphFindResultHeader {
        next_cursor,
        count: things.len() as u32,
    };

    let mut buf = postcard::to_allocvec(&header).ok()?;

    for thing in things {
        let mut thing_bytes = postcard::to_allocvec(&thing).ok()?;
        buf.append(&mut thing_bytes);
    }

    Some(buf)
}

pub fn export_watch_events(id: WatchId) -> Option<Vec<u8>> {
    let events = with_store(|store| store.poll_watch(id));
    postcard::to_allocvec(&events).ok()
}

fn emit_thing_event(thing: &GraphThing) {
    let mut payload = BTreeMap::new();
    payload.insert(canon::ID, Value::Uuid(thing.id));
    payload.insert(canon::KIND, Value::Symbol(thing.kind));
    payload.insert(canon::FIELDS, Value::Map(thing.fields.clone()));
    payload.insert(canon::OWNER, Value::Uuid(thing.owner));
    payload.insert(canon::REVISION, Value::U64(thing.revision));
    let _ = journal::emit_data(canon::THING_CREATED, Value::Map(payload));
}

fn emit_edge_event(edge: &GraphEdge) {
    let mut payload = BTreeMap::new();
    payload.insert(canon::SRC, Value::Uuid(edge.src));
    payload.insert(canon::DST, Value::Uuid(edge.dst));
    payload.insert(canon::PREDICATE, Value::Symbol(edge.pred));
    payload.insert(canon::OWNER, Value::Uuid(edge.owner));
    if !edge.props.is_empty() {
        payload.insert(canon::FIELDS, Value::Map(edge.props.clone()));
    }
    payload.insert(canon::REVISION, Value::U64(edge.revision));
    let _ = journal::emit_data(canon::EDGE_ADDED, Value::Map(payload));
}

fn derive_uuid(kind: Symbol, fields: &BTreeMap<Symbol, Value>) -> Uuid {
    let mut name: Vec<u8> = Vec::new();
    name.extend_from_slice(&kind.0.to_be_bytes());
    if let Ok(buf) = postcard::to_allocvec(fields) {
        name.extend_from_slice(&buf);
    }
    Uuid::new_v5(&Uuid::NAMESPACE_OID, &name)
}

fn reflect_thing_side_effects(thing: &GraphThing) {
    if thing.kind != canon::WRITE {
        return;
    }

    if let Some(text) = extract_text(&Value::Map(thing.fields.clone())) {
        for byte in text.bytes() {
            crate::drivers::framebuffer::console_write_byte(byte);
        }
    }
}

fn extract_text(value: &Value) -> Option<alloc::string::String> {
    match value {
        Value::Text(s) => Some(s.clone()),
        Value::Bytes(b) => core::str::from_utf8(b)
            .ok()
            .map(alloc::string::String::from),
        Value::Map(m) => m.get(&canon::TEXT).and_then(extract_text),
        _ => None,
    }
}

// ============================================================================
// Bundle Lifecycle Management
// ============================================================================

/// Bundle type classification for the bundle lifecycle.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BundleType {
    /// Device driver bundle
    Driver,
    /// User application bundle
    App,
    /// Window compositor bundle
    Compositor,
}

impl BundleType {
    /// Convert the bundle type to its corresponding symbol.
    pub fn to_symbol(self) -> Symbol {
        match self {
            BundleType::Driver => canon::DRIVER,
            BundleType::App => canon::APP,
            BundleType::Compositor => canon::COMPOSITOR,
        }
    }

    /// Attempt to infer bundle type from a module name.
    /// Uses suffix matching for more precise classification:
    /// - Names ending with "_driver" or "driver" are classified as Driver
    /// - Names ending with "_compositor" or "compositor" are classified as Compositor
    /// - Everything else defaults to App
    pub fn from_name(name: &str) -> Self {
        // Check for driver suffix patterns (more specific matching)
        if name.ends_with("_driver") || name == "driver" || name.ends_with("driver") {
            BundleType::Driver
        } else if name.ends_with("_compositor")
            || name == "compositor"
            || name.ends_with("compositor")
        {
            BundleType::Compositor
        } else {
            BundleType::App
        }
    }
}

/// Create a new bundle node with proper type information and return its ID.
/// This is the primary entry point for creating bundles with the full lifecycle.
pub fn create_bundle(name: &str, bundle_type: BundleType, version: Option<&str>) -> BundleId {
    let bundle_id = Uuid::new_v5(&Uuid::NAMESPACE_OID, name.as_bytes());
    create_bundle_with_id(bundle_id, name, bundle_type, version)
}

/// Create a bundle node with an explicit ID.
/// Useful when the BundleId has already been determined externally.
pub fn create_bundle_with_id(
    bundle_id: BundleId,
    name: &str,
    bundle_type: BundleType,
    version: Option<&str>,
) -> BundleId {
    let mut fields = BTreeMap::new();
    fields.insert(canon::ID, Value::Uuid(bundle_id));
    fields.insert(canon::NAME, Value::Text(name.into()));
    fields.insert(canon::TYPE, Value::Symbol(bundle_type.to_symbol()));
    fields.insert(canon::STATUS, Value::Symbol(canon::INIT));
    if let Some(v) = version {
        fields.insert(canon::VERSION, Value::Text(v.into()));
    }

    let req = GraphFiatRequest {
        id: Some(bundle_id),
        kind: canon::BUNDLE,
        fields,
    };

    // Use the bundle itself as owner (bundles own themselves)
    fiat_for_bundle(bundle_id, req);
    bundle_id
}

/// Look up an existing bundle by name, returning its BundleId if it exists.
pub fn lookup_bundle(name: &str) -> Option<BundleId> {
    let expected_id = Uuid::new_v5(&Uuid::NAMESPACE_OID, name.as_bytes());
    with_store(|store| {
        store.latest(&expected_id).map(|thing| {
            if thing.kind == canon::BUNDLE {
                Some(thing.id)
            } else {
                None
            }
        })?
    })
}

/// Get or create a bundle by name. If the bundle exists, returns its ID.
/// If it doesn't exist, creates a new bundle with the inferred type.
///
/// Note: This function is not atomic. During kernel boot when bundles are
/// initialized single-threaded, this is safe. If used in a concurrent context,
/// callers should ensure proper synchronization.
pub fn get_or_create_bundle(name: &str) -> BundleId {
    if let Some(id) = lookup_bundle(name) {
        return id;
    }
    let bundle_type = BundleType::from_name(name);
    create_bundle(name, bundle_type, None)
}

/// Grant initial capabilities to a bundle for a target node.
/// This is used when launching a bundle to give it access to its initial resources.
pub fn grant_initial_capability(bundle: BundleId, target: Uuid, capability: Symbol) -> bool {
    let req = GrantCapabilityRequest {
        grantee: bundle,
        target,
        capability,
    };
    // Kernel grants the capability
    grant_capability(KERNEL_BUNDLE_ID, req)
}
