#![no_std]

extern crate alloc;

use alloc::collections::{BTreeMap, BTreeSet};
use alloc::string::String;
use alloc::vec::Vec;
use core::cell::UnsafeCell;
use core::fmt;
use core::sync::atomic::{AtomicU64, AtomicU8, Ordering};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

pub type BundleId = Uuid;

pub const KERNEL_BUNDLE_ID: BundleId = Uuid::from_u128(0xfeed_cafe_dead_beef_cafe_babe_0000_0001);

#[derive(Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize, Debug)]
pub struct Symbol(pub u32);

impl Symbol {
    pub const fn new(raw: u32) -> Self {
        Symbol(raw)
    }

    pub const fn raw(self) -> u32 {
        self.0
    }
}

impl From<Symbol> for String {
    fn from(s: Symbol) -> Self {
        let val = s.0;
        let c1 = ((val >> 16) & 0xFF) as u8;
        let c2 = ((val >> 8) & 0xFF) as u8;
        let c3 = (val & 0xFF) as u8;

        let mut bytes = Vec::new();
        if c1 != 0 {
            bytes.push(c1);
        }
        if c2 != 0 {
            bytes.push(c2);
        }
        if c3 != 0 {
            bytes.push(c3);
        }

        if let Ok(s) = core::str::from_utf8(&bytes) {
            String::from(s)
        } else {
            alloc::format!("{:x}", val)
        }
    }
}

pub const fn cc(a: char, b: char) -> Symbol {
    Symbol(((a as u32) << 16) | ((b as u32) << 8))
}

pub const fn canon(a: u8, b: u8, c: u8) -> Symbol {
    Symbol(((a as u32) << 16) | ((b as u32) << 8) | (c as u32))
}

pub const fn from_char(c: char) -> Symbol {
    Symbol(c as u32)
}

pub const fn from_u16(raw: u16) -> Symbol {
    Symbol(raw as u32)
}

pub type Map = BTreeMap<Symbol, Value>;

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
    Blob(Vec<u8>),
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub struct FramebufferGeometry {
    pub width: u32,
    pub height: u32,
    pub pitch: u32,
    pub bpp: u16,
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

    pub fn as_map(&self) -> Option<&Map> {
        match self {
            Value::Map(m) => Some(m),
            _ => None,
        }
    }

    pub fn as_uuid(&self) -> Option<Uuid> {
        match self {
            Value::Uuid(id) => Some(*id),
            _ => None,
        }
    }

    pub fn as_symbol(&self) -> Option<Symbol> {
        match self {
            Value::Symbol(sym) => Some(*sym),
            _ => None,
        }
    }

    pub fn as_u64(&self) -> Option<u64> {
        match self {
            Value::U64(v) => Some(*v),
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

    pub fn as_text(&self) -> Option<&str> {
        match self {
            Value::Text(s) => Some(s.as_str()),
            _ => None,
        }
    }
}

impl fmt::Display for Value {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Value::Null => f.write_str("null"),
            Value::Bool(v) => write!(f, "{}", v),
            Value::U64(v) => write!(f, "{}", v),
            Value::I64(v) => write!(f, "{}", v),
            Value::Bytes(b) => write!(f, "bytes({})", b.len()),
            Value::Blob(b) => write!(f, "blob({})", b.len()),
            Value::Symbol(sym) => write!(f, "{}", sym.raw()),
            Value::Uuid(id) => write!(f, "{}", id),
            Value::Text(s) => write!(f, "\"{}\"", s),
            Value::Map(m) => {
                f.write_str("{")?;
                let mut first = true;
                for (k, v) in m.iter() {
                    if !first {
                        f.write_str(", ")?;
                    }
                    first = false;
                    write!(f, "{}: {}", k.raw(), v)?;
                }
                f.write_str("}")
            }
            Value::List(items) => {
                f.write_str("[")?;
                let mut first = true;
                for v in items {
                    if !first {
                        f.write_str(", ")?;
                    }
                    first = false;
                    write!(f, "{}", v)?;
                }
                f.write_str("]")
            }
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
    pub fields: Map,
    pub owner: BundleId,
    pub revision: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphEdge {
    pub id: Uuid,
    pub src: Uuid,
    pub pred: String,
    pub dst: Uuid,
    pub props: Map,
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
    #[serde(default)]
    pub labels: Vec<Symbol>,
    pub fields: Map,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphThatRequest {
    pub src: Uuid,
    pub pred: String,
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
    pub pred: String,
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SharedBufferSpec {
    pub id: Option<Uuid>,
    pub size_bytes: u64,
    pub kind: Symbol,
    pub usage: Symbol,
    #[serde(default)]
    pub addr: Option<u64>,
    #[serde(default)]
    pub props: Map,
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
    pub props: Map,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GrantCapabilityRequest {
    pub grantee: BundleId,
    pub target: Uuid,
    pub capability: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[repr(C)]
pub struct GraphFindByKind {
    pub kind_ptr: u64,
    pub kind_len: u64,
    pub cursor: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[repr(C)]
pub struct GraphFindResultHeader {
    pub next_cursor: u64,
    pub count: u32,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum SysError {
    Unknown,
    InvalidArgument,
    NotFound,
    AccessDenied,
    OutOfMemory,
    Backend(u32),
}

pub type SysResult<T> = core::result::Result<T, SysError>;

pub trait ThingSys {
    fn graph_fiat(&self, req: GraphFiatRequest) -> SysResult<GraphThing>;
    fn graph_link(&self, req: GraphLinkRequest) -> SysResult<GraphEdge>;
    fn graph_get(&self, req: GraphGetRequest) -> SysResult<Vec<GraphThing>>;
    fn graph_props_get(&self, req: GraphPropsGetRequest) -> SysResult<Map>;
    fn graph_props_set(&self, req: GraphPropsRequest) -> SysResult<()>;
}

pub type WatchId = u64;

pub type ThingId = Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThingFields(pub Map);

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AbiRequest {
    Fiat {
        id: Option<ThingId>,
        kind: Symbol,
        #[serde(default)]
        labels: Vec<Symbol>,
        fields: Map,
    },
    Link {
        id: Option<ThingId>,
        from: ThingId,
        pred: String,
        to: ThingId,
        #[serde(default)]
        props: Map,
    },
    Get {
        id: ThingId,
    },
    Query {
        pattern: NodePattern,
    },
    FindByKind {
        kind: String,
        cursor: Option<u64>,
    },
    WatchRegister {
        pattern: NodePattern,
    },
    WatchUnregister {
        watch_id: WatchId,
    },
    WatchPoll {
        watch_id: WatchId,
        max_events: Option<u32>,
    },
    PropsGet {
        request: GraphPropsGetRequest,
    },
    PropsSet {
        request: GraphPropsRequest,
    },
    GrantCapability {
        request: GrantCapabilityRequest,
    },
    DevOpen {
        kind: u32,
        index: usize,
    },
    DevRead {
        handle: u64,
        len: usize,
    },
    DevWrite {
        handle: u64,
        data: Vec<u8>,
    },
    IrqBind {
        device: Uuid,
        line: u8,
    },
    IrqAck {
        handle: u64,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AbiResponse {
    Fiat {
        thing: GraphThing,
    },
    Link {
        edge: Option<GraphEdge>,
    },
    Get {
        thing: Option<GraphThing>,
    },
    Query {
        things: Vec<GraphThing>,
    },
    Find {
        things: Vec<GraphThing>,
        next_cursor: Option<u64>,
    },
    WatchRegistered {
        watch_id: WatchId,
    },
    WatchUnregistered,
    WatchEvents {
        events: GraphWatchBatch,
    },
    Props {
        props: Map,
    },
    CapabilityGranted {
        granted: bool,
    },
    DevOpened {
        handle: Option<u64>,
    },
    DevRead {
        data: Vec<u8>,
    },
    DevWritten {
        len: usize,
    },
    IrqBound {
        handle: Option<u64>,
    },
    IrqAcked,
    Error {
        message: String,
    },
}

pub trait ThingRuntime: Sync {
    fn call(&self, req: AbiRequest) -> AbiResponse;
}

struct RuntimeCell {
    state: AtomicU8,
    value: UnsafeCell<Option<&'static dyn ThingRuntime>>,
}

unsafe impl Sync for RuntimeCell {}

const STATE_UNINIT: u8 = 0;
const STATE_INITING: u8 = 1;
const STATE_INITED: u8 = 2;

impl RuntimeCell {
    const fn new() -> Self {
        Self {
            state: AtomicU8::new(STATE_UNINIT),
            value: UnsafeCell::new(None),
        }
    }

    fn set(&self, runtime: &'static dyn ThingRuntime) -> Result<(), &'static dyn ThingRuntime> {
        match self.state.compare_exchange(
            STATE_UNINIT,
            STATE_INITING,
            Ordering::Acquire,
            Ordering::Relaxed,
        ) {
            Ok(_) => {
                unsafe { *self.value.get() = Some(runtime) };
                self.state.store(STATE_INITED, Ordering::Release);
                Ok(())
            }
            Err(_) => Err(runtime),
        }
    }

    fn get(&self) -> Option<&'static dyn ThingRuntime> {
        if self.state.load(Ordering::Acquire) != STATE_INITED {
            return None;
        }
        unsafe { (*self.value.get()).clone() }
    }

    fn is_set(&self) -> bool {
        self.state.load(Ordering::Acquire) == STATE_INITED
    }
}

static RUNTIME: RuntimeCell = RuntimeCell::new();

pub fn set_runtime(runtime: &'static dyn ThingRuntime) -> Result<(), &'static dyn ThingRuntime> {
    RUNTIME.set(runtime)
}

pub fn runtime() -> &'static dyn ThingRuntime {
    RUNTIME.get().expect("thing runtime not set")
}

pub fn runtime_is_set() -> bool {
    RUNTIME.is_set()
}

static UUID_COUNTER: AtomicU64 = AtomicU64::new(0);

pub fn next_uuid() -> Uuid {
    let id = UUID_COUNTER.fetch_add(1, Ordering::Relaxed);
    Uuid::from_u128(((id as u128) << 64) | (id as u128))
}

#[cfg(test)]
extern crate std;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn value_round_trips() {
        let mut map = Map::new();
        map.insert(Symbol::new(1), Value::U64(7));
        let value = Value::Map(map);
        let encoded = postcard::to_allocvec(&value).expect("encode");
        let decoded: Value = postcard::from_bytes(&encoded).expect("decode");
        assert_eq!(decoded, value);
    }

    #[test]
    fn graph_change_reports_revision() {
        let thing = GraphThing {
            id: Uuid::nil(),
            kind: Symbol::new(0),
            labels: BTreeSet::new(),
            fields: Map::new(),
            owner: Uuid::nil(),
            revision: 42,
        };
        let change = GraphChange::Thing(thing);
        assert_eq!(change.revision(), 42);
    }
}
