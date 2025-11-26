use crate::telemetry::canon;
use crate::telemetry::canon::Symbol;
use crate::telemetry::journal::{self, Value};
use alloc::collections::{BTreeMap, BTreeSet};
use alloc::vec::Vec;
use serde::{Deserialize, Serialize};
use spin::Mutex;
use uuid::Uuid;

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
    pub fields: BTreeMap<Symbol, Value>,
    pub revision: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphEdge {
    pub src: Uuid,
    pub pred: Symbol,
    pub dst: Uuid,
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
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WatchQuery {
    pub kind: Option<Symbol>,
    pub src: Option<Uuid>,
    pub dst: Option<Uuid>,
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
    query: WatchQuery,
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

    pub fn register_watch(&mut self, query: WatchQuery) -> WatchId {
        let id = self.next_watch_id;
        self.next_watch_id += 1;
        self.watches.insert(
            id,
            Watch {
                id,
                query,
                queue: Vec::new(),
            },
        );
        id
    }

    pub fn find_by_kind(&self, kind: &str, cursor: u64) -> (Vec<GraphThing>, u64) {
        let symbol = if kind == "window" {
            canon::WINDOW
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
    pub fn fiat(&mut self, request: GraphFiatRequest) -> GraphThing {
        let id = request
            .id
            .unwrap_or_else(|| derive_uuid(request.kind, &request.fields));
        let revision = self.next_revision();
        let thing = GraphThing {
            id,
            kind: request.kind,
            fields: request.fields,
            revision,
        };

        self.insert_thing(thing.clone());
        // self.record_change(GraphChange::Thing(thing.clone()));
        self.notify_watches(&GraphChange::Thing(thing.clone()));
        emit_thing_event(&thing);
        reflect_thing_side_effects(&thing);
        thing
    }

    pub fn that(&mut self, request: GraphThatRequest) -> u64 {
        let revision = self.next_revision();
        let edge = GraphEdge {
            src: request.src,
            pred: request.pred,
            dst: request.dst,
            revision,
        };
        self.insert_edge(edge.clone());
        // self.record_change(GraphChange::Edge(edge.clone()));
        self.notify_watches(&GraphChange::Edge(edge.clone()));
        emit_edge_event(&edge);
        revision
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
        for watch in self.watches.values_mut() {
            if Self::matches(&watch.query, change) {
                watch.queue.push(change.clone());
                if watch.queue.len() > MAX_WATCH_QUEUE {
                    // Drop oldest
                    watch.queue.remove(0);
                }
            }
        }
    }

    fn matches(query: &WatchQuery, change: &GraphChange) -> bool {
        match change {
            GraphChange::Thing(t) => {
                if let Some(kind) = query.kind {
                    if t.kind != kind {
                        return false;
                    }
                }
                if let Some(id) = query.src {
                    // For things, src filter might mean "is this thing"
                    if t.id != id {
                        return false;
                    }
                }
                // dst filter doesn't apply to things usually, unless we define it
                true
            }
            GraphChange::Edge(e) => {
                // Edges don't have a "kind" in the same way, but they have a predicate.
                // If query.kind is set, maybe we check predicate? Or we need a predicate filter.
                // The current WatchQuery has `kind`, `src`, `dst`.
                // Let's assume `kind` maps to `pred` for edges if we want to filter by edge type.
                if let Some(kind) = query.kind {
                    if e.pred != kind {
                        return false;
                    }
                }
                if let Some(src) = query.src {
                    if e.src != src {
                        return false;
                    }
                }
                if let Some(dst) = query.dst {
                    if e.dst != dst {
                        return false;
                    }
                }
                true
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

pub fn fiat(request: GraphFiatRequest) -> GraphThing {
    with_store(|store| store.fiat(request))
}

pub fn that(request: GraphThatRequest) -> u64 {
    with_store(|store| store.that(request))
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

pub fn register_watch(query: WatchQuery) -> WatchId {
    with_store(|store| store.register_watch(query))
}

pub fn export_find_by_kind_bytes(kind: &str, cursor: u64) -> Option<Vec<u8>> {
    let (things, next_cursor) = with_store(|store| store.find_by_kind(kind, cursor));

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
    payload.insert(canon::REVISION, Value::U64(thing.revision));
    let _ = journal::emit_data(canon::THING_CREATED, Value::Map(payload));
}

fn emit_edge_event(edge: &GraphEdge) {
    let mut payload = BTreeMap::new();
    payload.insert(canon::SRC, Value::Uuid(edge.src));
    payload.insert(canon::DST, Value::Uuid(edge.dst));
    payload.insert(canon::PREDICATE, Value::Symbol(edge.pred));
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
