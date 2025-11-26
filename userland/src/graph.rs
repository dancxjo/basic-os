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
    let mut buf = vec![0u8; MAX_SNAPSHOT_BYTES];
    let len = sys::graph_snapshot_raw(&mut buf);
    if len == !0 {
        return None;
    }
    postcard::from_bytes(&buf[..len as usize]).ok()
}

pub fn find_by_kind(kind: &str) -> Vec<GraphThing> {
    let mut results = Vec::new();
    let mut cursor = 0;
    let mut buf = vec![0u8; 64 * 1024]; // 64KB buffer

    loop {
        let req = sys::GraphFindByKind {
            kind_ptr: kind.as_ptr() as u64,
            kind_len: kind.len() as u64,
            cursor,
        };

        let bytes_written = sys::graph_find_by_kind_raw(&req, &mut buf);
        if bytes_written == !0 {
            break;
        }

        let slice = &buf[..bytes_written as usize];
        if let Ok((header, rest)) = postcard::take_from_bytes::<sys::GraphFindResultHeader>(slice) {
            let mut remaining = rest;
            for _ in 0..header.count {
                if let Ok((thing, next)) = postcard::take_from_bytes::<GraphThing>(remaining) {
                    results.push(thing);
                    remaining = next;
                } else {
                    break;
                }
            }

            if header.next_cursor == 0 {
                break;
            }
            cursor = header.next_cursor;
        } else {
            break;
        }
    }
    results
}

pub trait Thingable: Sized {
    fn load(thing: &GraphThing) -> Option<Self>;
}

pub fn load_thing<T: Thingable>(id: Uuid) -> Option<T> {
    let mut buf = vec![0u8; 4096];
    let len = sys::graph_get_raw(id.as_bytes(), &mut buf);
    if len == !0 {
        return None;
    }
    let thing: GraphThing = postcard::from_bytes(&buf[..len as usize]).ok()?;
    T::load(&thing)
}

pub fn update_thing<T: Thingable>(_id: Uuid, _thing: T) {
    // Placeholder
}

#[derive(Debug, Clone)]
pub struct Window {
    pub id: Uuid,
    pub width: u64,
    pub height: u64,
    pub title: String,
    pub x: u64,
    pub y: u64,
}

impl Thingable for Window {
    fn load(thing: &GraphThing) -> Option<Self> {
        if thing.kind != canon::WINDOW {
            return None;
        }
        let width = thing.fields.get(&canon::WIDTH).and_then(|v| v.as_u64()).unwrap_or(0);
        let height = thing.fields.get(&canon::HEIGHT).and_then(|v| v.as_u64()).unwrap_or(0);
        let title = thing.fields.get(&canon::TITLE).and_then(|v| extract_text(v)).unwrap_or_default();
        let x = thing.fields.get(&canon::X).and_then(|v| v.as_u64()).unwrap_or(0);
        let y = thing.fields.get(&canon::Y).and_then(|v| v.as_u64()).unwrap_or(0);
        Some(Window {
            id: thing.id,
            width,
            height,
            title,
            x,
            y,
        })
    }
}

impl Window {
    pub fn to_fields(&self) -> Map {
        let mut map = Map::new();
        map.insert(canon::WIDTH, Value::U64(self.width));
        map.insert(canon::HEIGHT, Value::U64(self.height));
        map.insert(canon::TITLE, Value::Text(self.title.clone()));
        map.insert(canon::X, Value::U64(self.x));
        map.insert(canon::Y, Value::U64(self.y));
        map
    }
}

pub fn watch(query: WatchQuery) -> Option<WatchHandle> {
    if let Ok(buf) = postcard::to_allocvec(&query) {
        let id = sys::watch_register_raw(&buf);
        if id != !0 {
            return Some(WatchHandle { id });
        }
    }
    None
}

pub fn poll_watch(handle: &WatchHandle) -> Vec<GraphChange> {
    let mut buf = vec![0u8; 64 * 1024];
    let len = sys::watch_poll_raw(handle.id, &mut buf);
    if len == !0 {
        return Vec::new();
    }
    postcard::from_bytes(&buf[..len as usize]).unwrap_or_default()
}

pub fn fiat_thing<T>(thing: &T) -> Uuid {
    // Placeholder
    Uuid::nil()
}

pub fn load_things_of_kind<T: Thingable>() -> Vec<(Uuid, T)> {
    if let Some(snapshot) = graph_snapshot() {
        let mut results = Vec::new();
        for thing in snapshot.things {
            if let Some(obj) = T::load(&thing) {
                results.push((thing.id, obj));
            }
        }
        results
    } else {
        Vec::new()
    }
}

pub fn log_args(args: fmt::Arguments) {
    // Placeholder for logging
}
