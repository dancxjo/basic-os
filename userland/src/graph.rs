use alloc::collections::BTreeMap;
use alloc::string::{String, ToString};
use alloc::vec;
use alloc::vec::Vec;
use core::fmt;

use crate::{canon, sys, Symbol};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

pub type Map = BTreeMap<Symbol, Value>;

// Snapshot buffers are small; guard against bogus sizes coming from the kernel.
const MAX_SNAPSHOT_BYTES: usize = 1 << 20; // 1 MiB upper bound

pub fn map() -> Map {
    BTreeMap::new()
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum Value {
    Null,
    Bool(bool),
    U64(u64),
    I64(i64),
    Bytes(Vec<u8>),
    Symbol(Symbol),
    Uuid(Uuid),
    Text(String),
    Map(Map),
    List(Vec<Value>),
}

impl Value {
    pub fn text(s: impl Into<String>) -> Self {
        Value::Text(s.into())
    }

    pub fn symbol(sym: Symbol) -> Self {
        Value::Symbol(sym)
    }

    pub fn uuid(id: Uuid) -> Self {
        Value::Uuid(id)
    }

    pub fn map(map: Map) -> Self {
        Value::Map(map)
    }

    pub fn as_uuid(&self) -> Option<Uuid> {
        match self {
            Value::Uuid(id) => Some(*id),
            _ => None,
        }
    }

    pub fn as_symbol(&self) -> Option<Symbol> {
        match self {
            Value::Symbol(s) => Some(*s),
            _ => None,
        }
    }

    pub fn as_u64(&self) -> Option<u64> {
        match self {
            Value::U64(v) => Some(*v),
            _ => None,
        }
    }

    pub fn as_map(&self) -> Option<&Map> {
        match self {
            Value::Map(m) => Some(m),
            _ => None,
        }
    }

    pub fn as_text(&self) -> Option<&str> {
        match self {
            Value::Text(s) => Some(s.as_str()),
            _ => None,
        }
    }

    pub fn as_i64(&self) -> Option<i64> {
        match self {
            Value::I64(v) => Some(*v),
            _ => None,
        }
    }

    pub fn as_bool(&self) -> Option<bool> {
        match self {
            Value::Bool(v) => Some(*v),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Event {
    pub timestamp: u64,
    pub kind: Symbol,
    pub data: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphFiatRequest {
    pub id: Option<Uuid>,
    pub kind: Symbol,
    pub fields: Map,
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

pub struct WatchHandle {
    pub id: u64,
}

pub fn extract_text(value: &Value) -> Option<String> {
    match value {
        Value::Text(s) => Some(s.clone()),
        Value::Bytes(b) => core::str::from_utf8(b).ok().map(ToString::to_string),
        Value::Map(m) => m.get(&canon::TEXT).and_then(extract_text),
        _ => None,
    }
}

/// Bring a Thing into existence in the graph. Returns the Thing ID that was declared.
pub fn fiat(id: Option<Uuid>, kind: Symbol, fields: Map) -> Uuid {
    let id = id.unwrap_or_else(|| {
        // Derive a stable UUID from the Thing kind and fields so callers do not need randomness.
        let mut name: Vec<u8> = Vec::new();
        name.extend_from_slice(&kind.0.to_be_bytes());
        if let Ok(buf) = postcard::to_allocvec(&fields) {
            name.extend_from_slice(&buf);
        }
        crate::simple_uuid(&name)
    });
    let req = GraphFiatRequest {
        id: Some(id),
        kind,
        fields,
    };
    if let Ok(buf) = postcard::to_allocvec(&req) {
        let _ = sys::graph_fiat_raw(&buf);
    }
    id
}

/// Add an edge between two Things in the graph.
pub fn that(src: Uuid, pred: Symbol, dst: Uuid, revision: u64) {
    let req = GraphThatRequest {
        src,
        pred,
        dst,
        revision_hint: revision,
    };
    if let Ok(buf) = postcard::to_allocvec(&req) {
        let _ = sys::graph_link_raw(&buf);
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphThing {
    pub id: Uuid,
    pub kind: Symbol,
    pub fields: Map,
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

pub fn graph_snapshot() -> Option<GraphSnapshot> {
    let mut buf = vec![0u8; 4096];
    let needed = sys::graph_snapshot_raw(&mut buf) as usize;
    if needed == 0 {
        return Some(GraphSnapshot {
            revision: 0,
            thing_count: 0,
            edge_count: 0,
            things: Vec::new(),
            edges: Vec::new(),
        });
    }
    if needed > MAX_SNAPSHOT_BYTES {
        return None;
    }
    if needed > buf.len() {
        buf.resize(needed, 0);
    }
    let written = sys::graph_snapshot_raw(&mut buf) as usize;
    if written == 0 || written > buf.len() {
        return None;
    }
    postcard::from_bytes::<GraphSnapshot>(&buf[..written]).ok()
}

pub fn watch(query: WatchQuery) -> Option<WatchHandle> {
    let buf = postcard::to_allocvec(&query).ok()?;
    let id = sys::watch_register_raw(&buf);
    if id == !0 {
        None
    } else {
        Some(WatchHandle { id })
    }
}

pub fn poll_watch(handle: &WatchHandle) -> Vec<GraphChange> {
    let mut buf = vec![0u8; 4096];
    let needed = sys::watch_poll_raw(handle.id, &mut buf) as usize;
    if needed == 0 {
        return Vec::new();
    }
    if needed > buf.len() {
        buf.resize(needed, 0);
    }
    let written = sys::watch_poll_raw(handle.id, &mut buf) as usize;
    if written == 0 || written > buf.len() {
        return Vec::new();
    }
    postcard::from_bytes::<Vec<GraphChange>>(&buf[..written]).unwrap_or_default()
}

pub fn graph_get(id: Uuid) -> Option<GraphThing> {
    let mut buf = vec![0u8; 4096];
    let id_bytes = id.into_bytes();
    let needed = sys::graph_get_raw(&id_bytes, &mut buf) as usize;
    if needed == 0 {
        return None;
    }
    if needed > MAX_SNAPSHOT_BYTES {
        return None;
    }
    if needed > buf.len() {
        buf.resize(needed, 0);
    }
    let written = sys::graph_get_raw(&id_bytes, &mut buf) as usize;
    if written == 0 || written > buf.len() {
        return None;
    }
    postcard::from_bytes::<GraphThing>(&buf[..written]).ok()
}

pub fn log_args(args: fmt::Arguments<'_>) {
    let mut buf = String::new();
    let _ = fmt::write(&mut buf, args);
    let mut fields = Map::new();
    fields.insert(canon::TEXT, Value::Text(buf));
    fiat(None, canon::WRITE, fields);
}

pub trait Thingable: Sized {
    fn kind() -> Symbol;
    fn to_fields(&self) -> Map;
    fn from_fields(fields: &Map) -> Option<Self>;
}

pub fn fiat_thing<T: Thingable>(value: &T) -> Uuid {
    let fields = value.to_fields();
    fiat(None, T::kind(), fields)
}

pub fn load_thing<T: Thingable>(id: Uuid) -> Option<T> {
    let thing = graph_get(id)?;
    T::from_fields(&thing.fields)
}

pub fn load_things_of_kind<T: Thingable>() -> Vec<(Uuid, T)> {
    let snapshot = match graph_snapshot() {
        Some(s) => s,
        None => return Vec::new(),
    };

    let mut latest: BTreeMap<Uuid, &GraphThing> = BTreeMap::new();
    for thing in &snapshot.things {
        if thing.kind == T::kind() {
            latest
                .entry(thing.id)
                .and_modify(|e| {
                    if thing.revision > e.revision {
                        *e = thing;
                    }
                })
                .or_insert(thing);
        }
    }

    latest
        .into_iter()
        .filter_map(|(id, thing)| T::from_fields(&thing.fields).map(|t| (id, t)))
        .collect()
}

pub fn update_thing<T: Thingable>(id: Uuid, new_value: &T) {
    let fields = new_value.to_fields();
    fiat(Some(id), T::kind(), fields);
}

#[derive(Debug, Clone, PartialEq)]
pub struct Window {
    pub title: String,
    pub x: u64,
    pub y: u64,
    pub width: u64,
    pub height: u64,
}

impl Thingable for Window {
    fn kind() -> Symbol {
        canon::WINDOW
    }

    fn to_fields(&self) -> Map {
        let mut m = Map::new();
        m.insert(canon::TITLE, Value::Text(self.title.clone()));
        m.insert(canon::X, Value::U64(self.x));
        m.insert(canon::Y, Value::U64(self.y));
        m.insert(canon::WIDTH, Value::U64(self.width));
        m.insert(canon::HEIGHT, Value::U64(self.height));
        m
    }

    fn from_fields(fields: &Map) -> Option<Self> {
        Some(Window {
            title: fields.get(&canon::TITLE)?.as_text()?.to_string(),
            x: fields.get(&canon::X)?.as_u64()?,
            y: fields.get(&canon::Y)?.as_u64()?,
            width: fields.get(&canon::WIDTH)?.as_u64()?,
            height: fields.get(&canon::HEIGHT)?.as_u64()?,
        })
    }
}

#[cfg(test)]
mod tests {
    extern crate std;

    use super::*;

    #[test]
    fn test_window_round_trip() {
        let window = Window {
            title: "Test Window".to_string(),
            x: 100,
            y: 200,
            width: 800,
            height: 600,
        };

        let fields = window.to_fields();
        let decoded = Window::from_fields(&fields).expect("decode failed");

        assert_eq!(window, decoded);
    }

    #[test]
    fn test_graph_snapshot_decode() {
        let snapshot = GraphSnapshot {
            revision: 42,
            thing_count: 1,
            edge_count: 0,
            things: vec![GraphThing {
                id: Uuid::nil(),
                kind: canon::WINDOW,
                fields: {
                    let mut m = Map::new();
                    m.insert(canon::TITLE, Value::Text("Test".to_string()));
                    m.insert(canon::X, Value::U64(0));
                    m.insert(canon::Y, Value::U64(0));
                    m.insert(canon::WIDTH, Value::U64(100));
                    m.insert(canon::HEIGHT, Value::U64(100));
                    m
                },
                revision: 0,
            }],
            edges: vec![],
        };

        let encoded = postcard::to_allocvec(&snapshot).expect("encode failed");
        let decoded = postcard::from_bytes::<GraphSnapshot>(&encoded).expect("decode failed");

        assert_eq!(snapshot.revision, decoded.revision);
        assert_eq!(snapshot.things.len(), decoded.things.len());
        assert_eq!(snapshot.edges.len(), decoded.edges.len());
    }
}
