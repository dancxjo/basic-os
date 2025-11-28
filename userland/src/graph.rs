use alloc::collections::{BTreeMap, BTreeSet};
use alloc::string::{String, ToString};
use alloc::vec;
use alloc::vec::Vec;
use core::fmt;

use crate::{canon, sys, Symbol};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[cfg(test)]
extern crate std;

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
pub enum GraphGetRequest {
    Thing(Uuid),
    Pattern(NodePattern),
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
    let Ok(encoded) = postcard::to_allocvec(&GraphGetRequest::Pattern(pattern)) else {
        return Vec::new();
    };
    let len = sys::graph_get_raw(&encoded, &mut buf);
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

#[derive(Debug, Clone)]
pub struct SharedBuffer {
    pub id: Uuid,
    pub size_bytes: u64,
    pub kind: Symbol,
    pub usage: Symbol,
    pub addr: Option<u64>,
    pub owner: Option<Uuid>,
    pub props: Map,
}

impl SharedBuffer {
    pub fn to_fields(&self) -> Map {
        let mut fields = self.props.clone();
        fields.insert(canon::BYTES, Value::U64(self.size_bytes));
        fields.insert(canon::BUFFER_KIND, Value::Symbol(self.kind));
        fields.insert(canon::BUFFER_USAGE, Value::Symbol(self.usage));
        if let Some(addr) = self.addr {
            fields.insert(canon::ADDR, Value::U64(addr));
        }
        if let Some(owner) = self.owner {
            fields.insert(canon::OWNER, Value::Uuid(owner));
        }
        fields
    }
}

impl Thingable for SharedBuffer {
    fn kind() -> &'static str {
        "buffer.shared"
    }

    fn load(thing: &GraphThing) -> Option<Self> {
        if thing.kind != canon::SHARED_BUFFER {
            return None;
        }
        let size_bytes = thing
            .fields
            .get(&canon::BYTES)
            .and_then(|v| v.as_u64())
            .unwrap_or(0);
        let kind = thing
            .fields
            .get(&canon::BUFFER_KIND)
            .and_then(|v| v.as_symbol())?;
        let usage = thing
            .fields
            .get(&canon::BUFFER_USAGE)
            .and_then(|v| v.as_symbol())?;
        let addr = thing.fields.get(&canon::ADDR).and_then(|v| v.as_u64());
        let owner = thing.fields.get(&canon::OWNER).and_then(|v| v.as_uuid());

        Some(SharedBuffer {
            id: thing.id,
            size_bytes,
            kind,
            usage,
            addr,
            owner,
            props: thing.fields.clone(),
        })
    }
}

#[derive(Debug, Clone)]
pub struct QueueState {
    pub id: Uuid,
    pub buffer: Uuid,
    pub owner: Option<Uuid>,
    pub head: u64,
    pub tail: u64,
    pub has_data: bool,
    pub capacity: Option<u64>,
    pub props: Map,
}

impl QueueState {
    pub fn to_fields(&self) -> Map {
        let mut fields = self.props.clone();
        fields.insert(canon::BUFFER, Value::Uuid(self.buffer));
        fields.insert(canon::HEAD, Value::U64(self.head));
        fields.insert(canon::TAIL, Value::U64(self.tail));
        fields.insert(canon::HAS_DATA, Value::Bool(self.has_data));
        if let Some(capacity) = self.capacity {
            fields.insert(canon::CAPACITY, Value::U64(capacity));
        }
        if let Some(owner) = self.owner {
            fields.insert(canon::OWNER, Value::Uuid(owner));
        }
        fields
    }
}

impl Thingable for QueueState {
    fn kind() -> &'static str {
        "queue.state"
    }

    fn load(thing: &GraphThing) -> Option<Self> {
        if thing.kind != canon::QUEUE_STATE {
            return None;
        }
        let buffer = thing.fields.get(&canon::BUFFER)?.as_uuid()?;
        let owner = thing.fields.get(&canon::OWNER).and_then(|v| v.as_uuid());
        let head = thing
            .fields
            .get(&canon::HEAD)
            .and_then(|v| v.as_u64())
            .unwrap_or(0);
        let tail = thing
            .fields
            .get(&canon::TAIL)
            .and_then(|v| v.as_u64())
            .unwrap_or(0);
        let has_data = thing
            .fields
            .get(&canon::HAS_DATA)
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        let capacity = thing.fields.get(&canon::CAPACITY).and_then(|v| v.as_u64());

        Some(QueueState {
            id: thing.id,
            buffer,
            owner,
            head,
            tail,
            has_data,
            capacity,
            props: thing.fields.clone(),
        })
    }
}

pub fn declare_shared_buffer(buffer: SharedBuffer) -> Uuid {
    let fields = buffer.to_fields();
    fiat(Some(buffer.id), canon::SHARED_BUFFER, fields)
}

pub fn declare_queue_state(queue: QueueState) -> Uuid {
    let fields = queue.to_fields();
    fiat(Some(queue.id), canon::QUEUE_STATE, fields)
}

pub fn update_queue_state(
    node: Uuid,
    head: u64,
    tail: u64,
    has_data: bool,
    capacity: Option<u64>,
) -> bool {
    let mut props = map();
    props.insert(canon::HEAD, Value::U64(head));
    props.insert(canon::TAIL, Value::U64(tail));
    props.insert(canon::HAS_DATA, Value::Bool(has_data));
    if let Some(cap) = capacity {
        props.insert(canon::CAPACITY, Value::U64(cap));
    }
    set_props(GraphPropsRequest { node, props })
}

pub trait Thingable: Sized {
    fn kind() -> &'static str;
    fn load(thing: &GraphThing) -> Option<Self>;
}

pub fn load_thing<T: Thingable>(id: Uuid) -> Option<T> {
    let mut buf = vec![0u8; 4096];
    let Ok(encoded) = postcard::to_allocvec(&GraphGetRequest::Thing(id)) else {
        return None;
    };
    let len = sys::graph_get_raw(&encoded, &mut buf);
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
    pub z: i64,
    pub visible: bool,
    pub target: Option<Uuid>,
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
        let z = thing
            .fields
            .get(&canon::Z)
            .and_then(|v| v.as_i64())
            .unwrap_or(0);
        let visible = thing
            .fields
            .get(&canon::VISIBLE)
            .and_then(|v| v.as_bool())
            .unwrap_or(true);
        let target = thing.fields.get(&canon::TARGET).and_then(|v| v.as_uuid());
        Some(Window {
            id: thing.id,
            width,
            height,
            title,
            x,
            y,
            z,
            visible,
            target,
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
        map.insert(canon::Z, Value::I64(self.z));
        map.insert(canon::VISIBLE, Value::Bool(self.visible));
        if let Some(target) = self.target {
            map.insert(canon::TARGET, Value::Uuid(target));
        }
        map
    }
}

#[derive(Debug, Clone)]
pub struct Surface {
    pub id: Uuid,
    pub window: Option<Uuid>,
    pub dirty: bool,
    pub text: String,
    pub bitmap: Option<Vec<u8>>,
}

impl Thingable for Surface {
    fn kind() -> &'static str {
        "surface"
    }

    fn load(thing: &GraphThing) -> Option<Self> {
        if thing.kind != canon::SURFACE {
            return None;
        }
        let window = thing.fields.get(&canon::SRC).and_then(|v| v.as_uuid());
        let dirty = thing
            .fields
            .get(&canon::DIRTY)
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        let text = thing
            .fields
            .get(&canon::TEXT)
            .and_then(extract_text)
            .unwrap_or_default();
        let bitmap_bytes = match thing.fields.get(&canon::BITMAP) {
            Some(Value::Bytes(buf)) => Some(buf.clone()),
            _ => None,
        };

        Some(Surface {
            id: thing.id,
            window,
            dirty,
            text,
            bitmap: bitmap_bytes,
        })
    }
}

pub fn watch(query: WatchQuery) -> Option<WatchHandle> {
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
    watch_pattern(pattern)
}

pub fn watch_pattern(pattern: NodePattern) -> Option<WatchHandle> {
    if let Ok(buf) = postcard::to_allocvec(&pattern) {
        let id = sys::graph_watch_register_raw(&buf);
        if id != !0 {
            return Some(WatchHandle { id });
        }
    }
    None
}

pub fn poll_watch(handle: &WatchHandle) -> Vec<GraphChange> {
    let mut buf = vec![0u8; 64 * 1024];
    let len = sys::graph_watch_poll_raw(handle.id, &mut buf);
    if len == !0 {
        return Vec::new();
    }
    postcard::from_bytes(&buf[..len as usize]).unwrap_or_default()
}

pub fn fiat_thing<T>(_thing: &T) -> Uuid {
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

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::collections::BTreeSet;

    #[test]
    #[ignore = "requires heap initialization in kernel context"]
    fn shared_buffer_round_trips_through_thingable() {
        crate::heap::init_heap();
        let mut fields = map();
        fields.insert(canon::BYTES, Value::U64(128));
        fields.insert(canon::BUFFER_KIND, Value::Symbol(canon::RING));
        fields.insert(canon::BUFFER_USAGE, Value::Symbol(canon::PIPE_USAGE));
        fields.insert(canon::ADDR, Value::U64(0xdead_beefu64));

        let thing = GraphThing {
            id: Uuid::nil(),
            kind: canon::SHARED_BUFFER,
            labels: BTreeSet::new(),
            fields: fields.clone(),
            owner: Uuid::nil(),
            revision: 1,
        };

        let loaded = SharedBuffer::load(&thing).expect("shared buffer should decode");
        assert_eq!(loaded.size_bytes, 128);
        assert_eq!(loaded.kind, canon::RING);
        assert_eq!(loaded.usage, canon::PIPE_USAGE);
        assert_eq!(loaded.addr, Some(0xdead_beefu64));
        assert!(loaded.props.contains_key(&canon::BYTES));
    }

    #[test]
    #[ignore = "requires heap initialization in kernel context"]
    fn queue_state_round_trips_through_thingable() {
        crate::heap::init_heap();
        let mut fields = map();
        fields.insert(canon::BUFFER, Value::Uuid(Uuid::nil()));
        fields.insert(canon::HEAD, Value::U64(4));
        fields.insert(canon::TAIL, Value::U64(2));
        fields.insert(canon::HAS_DATA, Value::Bool(true));
        fields.insert(canon::CAPACITY, Value::U64(256));
        fields.insert(canon::OWNER, Value::Uuid(Uuid::nil()));

        let thing = GraphThing {
            id: Uuid::from_u128(2),
            kind: canon::QUEUE_STATE,
            labels: BTreeSet::new(),
            fields: fields.clone(),
            owner: Uuid::nil(),
            revision: 1,
        };

        let loaded = QueueState::load(&thing).expect("queue state should decode");
        assert_eq!(loaded.head, 4);
        assert_eq!(loaded.tail, 2);
        assert_eq!(loaded.capacity, Some(256));
        assert!(loaded.has_data);
        assert_eq!(loaded.buffer, Uuid::nil());
    }
}

pub fn log_args(_args: fmt::Arguments) {
    // Placeholder for logging
}
