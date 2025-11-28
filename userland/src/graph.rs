use alloc::collections::{BTreeMap, BTreeSet};
use alloc::string::{String, ToString};
use alloc::vec;
use alloc::vec::Vec;
use core::fmt;

use crate::{canon, sys, Symbol};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

pub type Map = BTreeMap<Symbol, Value>;

// Snapshot buffers are small; guard against bogus sizes coming from the kernel.
// const MAX_SNAPSHOT_BYTES: usize = 1 << 20; // 1 MiB upper bound

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
    #[serde(default)]
    pub props: Map,
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
    pub props: Map,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphNodeRequest {
    pub id: Option<Uuid>,
    pub labels: Vec<Symbol>,
    pub props: Map,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphLinkRequest {
    pub id: Option<Uuid>,
    pub kind: Symbol,
    pub from: Uuid,
    pub to: Uuid,
    #[serde(default)]
    pub props: Map,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphPropsRequest {
    pub node: Uuid,
    pub props: Map,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphPropsGetRequest {
    pub node: Uuid,
    pub keys: Vec<Symbol>,
}

/// Request to grant a capability from one bundle to another.
/// The granting bundle must own the target node or have the capability itself.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GrantCapabilityRequest {
    /// The bundle receiving the capability
    pub grantee: Uuid,
    /// The target node the capability applies to
    pub target: Uuid,
    /// The capability being granted (e.g., CAN_READ, CAN_WRITE, CAN_LINK)
    pub capability: Symbol,
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
        props: map(),
    };
    if let Ok(buf) = postcard::to_allocvec(&req) {
        let _ = sys::graph_link_raw(&buf);
    }
}

/// Grant a capability to another bundle.
/// Data capabilities (read/write/link) may be delegated only by the owner of
/// the target Thing. Hardware capabilities (IRQ/DMA/MMIO/PORT IO) are
/// kernel-only. Returns true if the capability was successfully granted.
pub fn grant_capability(grantee: Uuid, target: Uuid, capability: Symbol) -> bool {
    let req = GrantCapabilityRequest {
        grantee,
        target,
        capability,
    };
    if let Ok(buf) = postcard::to_allocvec(&req) {
        sys::grant_capability_raw(&buf) == 0
    } else {
        false
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphThing {
    pub id: Uuid,
    pub kind: Symbol,
    pub labels: BTreeSet<Symbol>,
    pub fields: Map,
    pub owner: Uuid,
    pub revision: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphEdge {
    pub id: Uuid,
    pub src: Uuid,
    pub pred: Symbol,
    pub dst: Uuid,
    pub props: Map,
    pub owner: Uuid,
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

pub fn get_nodes(pattern: NodePattern) -> Vec<GraphThing> {
    let mut buf = vec![0u8; 64 * 1024];
    let Ok(encoded) = postcard::to_allocvec(&pattern) else {
        return Vec::new();
    };
    let len = sys::graph_get_nodes_raw(&encoded, &mut buf);
    if len == !0 {
        return Vec::new();
    }
    postcard::from_bytes::<Vec<GraphThing>>(&buf[..len as usize]).unwrap_or_default()
}

pub fn get_props(request: GraphPropsGetRequest) -> Option<Map> {
    let Ok(encoded) = postcard::to_allocvec(&request) else {
        return None;
    };
    let mut buf = vec![0u8; 4096];
    let len = sys::graph_get_props_raw(&encoded, &mut buf);
    if len == !0 {
        return None;
    }
    postcard::from_bytes::<Map>(&buf[..len as usize]).ok()
}

pub fn set_props(request: GraphPropsRequest) -> bool {
    if let Ok(encoded) = postcard::to_allocvec(&request) {
        return sys::graph_set_props_raw(&encoded) == 0;
    }
    false
}

pub trait Thingable: Sized {
    fn kind() -> &'static str;
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
    fn kind() -> &'static str {
        "window"
    }

    fn load(thing: &GraphThing) -> Option<Self> {
        if thing.kind != canon::WINDOW {
            return None;
        }
        let width = thing
            .fields
            .get(&canon::WIDTH)
            .and_then(|v| v.as_u64())
            .unwrap_or(0);
        let height = thing
            .fields
            .get(&canon::HEIGHT)
            .and_then(|v| v.as_u64())
            .unwrap_or(0);
        let title = thing
            .fields
            .get(&canon::TITLE)
            .and_then(|v| extract_text(v))
            .unwrap_or_default();
        let x = thing
            .fields
            .get(&canon::X)
            .and_then(|v| v.as_u64())
            .unwrap_or(0);
        let y = thing
            .fields
            .get(&canon::Y)
            .and_then(|v| v.as_u64())
            .unwrap_or(0);
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
    let things = find_by_kind(T::kind());
    let mut results = Vec::new();
    for thing in things {
        if let Some(obj) = T::load(&thing) {
            results.push((thing.id, obj));
        }
    }
    results
}

pub fn log_args(args: fmt::Arguments) {
    // Placeholder for logging
}
