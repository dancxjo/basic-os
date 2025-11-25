use crate::telemetry::canon;
use crate::telemetry::canon::Symbol;
use crate::telemetry::journal::{Event, Value};
use alloc::{collections::btree_map::BTreeMap, vec::Vec};
use serde::{Deserialize, Serialize};
use spin::Mutex;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Thing {
    pub id: Uuid,
    pub kind: Symbol,
    pub fields: BTreeMap<Symbol, Value>,
    pub revision: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Edge {
    pub src: Uuid,
    pub pred: Symbol,
    pub dst: Uuid,
    pub revision: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Snapshot {
    pub revision: u64,
    pub thing_count: usize,
    pub edge_count: usize,
    pub things: Vec<Thing>,
    pub edges: Vec<Edge>,
}

#[derive(Default)]
pub struct Store {
    pub things: BTreeMap<Uuid, Vec<Thing>>,
    pub edges: Vec<Edge>,
}

impl Store {
    pub fn new() -> Self {
        Self {
            things: BTreeMap::new(),
            edges: Vec::new(),
        }
    }

    pub fn latest(&self, id: &Uuid) -> Option<&Thing> {
        self.things.get(id).and_then(|versions| versions.last())
    }

    pub fn replay_events(&mut self, events: &[Event]) {
        self.things.clear();
        self.edges.clear();
        for event in events {
            self.apply_event(event);
        }
    }

    pub(crate) fn apply_event(&mut self, event: &Event) {
        if event.kind == canon::THING_CREATED {
            self.ingest_thing(event);
        } else if event.kind == canon::EDGE_ADDED {
            self.ingest_edge(event);
        }
    }

    fn ingest_thing(&mut self, event: &Event) {
        let data = match event.data.as_map() {
            Some(m) => m,
            None => return,
        };

        let id = match data.get(&canon::ID).and_then(Value::as_uuid) {
            Some(id) => id,
            None => return,
        };
        let kind = match data.get(&canon::KIND).and_then(Value::as_symbol) {
            Some(k) => k,
            None => return,
        };
        let revision = data
            .get(&canon::REVISION)
            .and_then(Value::as_u64)
            .unwrap_or(0);
        let fields = data
            .get(&canon::FIELDS)
            .and_then(Value::as_map)
            .cloned()
            .unwrap_or_else(BTreeMap::new);

        let thing = Thing {
            id,
            kind,
            fields,
            revision,
        };

        let versions = self.things.entry(id).or_default();
        let should_add = versions
            .last()
            .map(|prev| thing.revision > prev.revision)
            .unwrap_or(true);
        if should_add {
            versions.push(thing);
        }
    }

    fn ingest_edge(&mut self, event: &Event) {
        let data = match event.data.as_map() {
            Some(m) => m,
            None => return,
        };
        let src = match data.get(&canon::SRC).and_then(Value::as_uuid) {
            Some(v) => v,
            None => return,
        };
        let dst = match data.get(&canon::DST).and_then(Value::as_uuid) {
            Some(v) => v,
            None => return,
        };
        let pred = match data.get(&canon::PREDICATE).and_then(Value::as_symbol) {
            Some(v) => v,
            None => return,
        };
        let revision = data
            .get(&canon::REVISION)
            .and_then(Value::as_u64)
            .unwrap_or(0);

        let edge = Edge {
            src,
            pred,
            dst,
            revision,
        };

        let is_new = self
            .edges
            .last()
            .map(|prev| edge.revision >= prev.revision)
            .unwrap_or(true);
        if is_new {
            self.edges.push(edge);
        }
    }

    pub fn snapshot(&self) -> Snapshot {
        let mut things = Vec::new();
        let mut revision = 0;

        for versions in self.things.values() {
            for thing in versions {
                revision = revision.max(thing.revision);
                things.push(thing.clone());
            }
        }

        for edge in &self.edges {
            revision = revision.max(edge.revision);
        }

        Snapshot {
            revision,
            thing_count: self.things.len(),
            edge_count: self.edges.len(),
            things,
            edges: self.edges.clone(),
        }
    }

    pub fn apply_snapshot(&mut self, snapshot: Snapshot) {
        self.things.clear();
        self.edges.clear();
        self.edges.extend(snapshot.edges.into_iter());

        for thing in snapshot.things.into_iter() {
            let versions = self.things.entry(thing.id).or_default();
            versions.push(thing);
            versions.sort_by_key(|t| t.revision);
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

pub fn snapshot() -> Snapshot {
    with_store(|store| store.snapshot())
}

pub fn apply_snapshot(snapshot: Snapshot) {
    with_store(|store| store.apply_snapshot(snapshot));
}

pub fn export_snapshot_bytes() -> Option<Vec<u8>> {
    let snapshot = snapshot();
    postcard::to_allocvec(&snapshot).ok()
}

pub fn import_snapshot_bytes(buf: &[u8]) -> Result<(), postcard::Error> {
    let snapshot: Snapshot = postcard::from_bytes(buf)?;
    apply_snapshot(snapshot);
    Ok(())
}
