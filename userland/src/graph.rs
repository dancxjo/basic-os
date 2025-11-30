use alloc::string::{String, ToString};
use alloc::vec;
use alloc::vec::Vec;
use core::fmt;

use crate::runtime;
use crate::{canon, sys, Symbol, Value};
use thing_abi::{
    AbiRequest, AbiResponse, GrantCapabilityRequest, GraphPropsGetRequest, GraphPropsRequest, Map,
    WatchQuery,
};
use uuid::Uuid;

pub use thing_abi::{GraphChange, GraphEdge, GraphThing, NodePattern};

// Snapshot buffers are small; guard against bogus sizes coming from the kernel.
// const MAX_SNAPSHOT_BYTES: usize = 1 << 20; // 1 MiB upper bound

pub fn map() -> Map {
    Map::new()
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
    runtime::ensure_kernel_runtime();
    let id = id.unwrap_or_else(|| {
        let mut name: Vec<u8> = Vec::new();
        name.extend_from_slice(&kind.0.to_be_bytes());
        if let Ok(buf) = postcard::to_allocvec(&fields) {
            name.extend_from_slice(&buf);
        }
        crate::simple_uuid(&name)
    });
    match runtime::runtime().call(AbiRequest::Fiat {
        id: Some(id),
        kind,
        labels: vec![kind],
        fields,
    }) {
        AbiResponse::Fiat { thing } => thing.id,
        AbiResponse::Error { .. } => id,
        _ => id,
    }
}

/// Add an edge between two Things in the graph.
pub fn that(src: Uuid, pred: Symbol, dst: Uuid, revision: u64) {
    runtime::ensure_kernel_runtime();
    let _ = runtime::runtime().call(AbiRequest::Link {
        id: None,
        from: src,
        rel: pred,
        to: dst,
        props: map(),
    });
}

/// Grant a capability to another bundle.
/// Data capabilities (read/write/link) may be delegated only by the owner of
/// the target Thing. Hardware capabilities (IRQ/DMA/MMIO/PORT IO) are
/// kernel-only. Returns true if the capability was successfully granted.
pub fn grant_capability(grantee: Uuid, target: Uuid, capability: Symbol) -> bool {
    runtime::ensure_kernel_runtime();
    matches!(
        runtime::runtime().call(AbiRequest::GrantCapability {
            request: GrantCapabilityRequest {
                grantee,
                target,
                capability,
            },
        }),
        AbiResponse::CapabilityGranted { granted: true }
    )
}

pub fn find_by_kind(kind: &str) -> Vec<GraphThing> {
    runtime::ensure_kernel_runtime();
    let mut results = Vec::new();
    let mut cursor = None;
    loop {
        let response = runtime::runtime().call(AbiRequest::FindByKind {
            kind: kind.to_string(),
            cursor,
        });
        match response {
            AbiResponse::Find {
                mut things,
                next_cursor,
            } => {
                results.append(&mut things);
                cursor = next_cursor;
                if cursor.is_none() {
                    break;
                }
            }
            AbiResponse::Query { mut things } => {
                results.append(&mut things);
                break;
            }
            _ => break,
        }
    }
    results
}

pub fn get_nodes(pattern: NodePattern) -> Vec<GraphThing> {
    runtime::ensure_kernel_runtime();
    match runtime::runtime().call(AbiRequest::Query { pattern }) {
        AbiResponse::Query { things } => things,
        AbiResponse::Find { things, .. } => things,
        _ => Vec::new(),
    }
}

pub fn get_props(request: GraphPropsGetRequest) -> Option<Map> {
    runtime::ensure_kernel_runtime();
    match runtime::runtime().call(AbiRequest::PropsGet { request }) {
        AbiResponse::Props { props } => Some(props),
        _ => None,
    }
}

pub fn set_props(request: GraphPropsRequest) -> bool {
    runtime::ensure_kernel_runtime();
    matches!(
        runtime::runtime().call(AbiRequest::PropsSet { request }),
        AbiResponse::Props { .. }
    )
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
    runtime::ensure_kernel_runtime();
    let response = runtime::runtime().call(AbiRequest::Get { id });
    let thing = match response {
        AbiResponse::Get { thing } => thing?,
        AbiResponse::Fiat { thing } => thing,
        _ => return None,
    };
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
    runtime::ensure_kernel_runtime();
    match runtime::runtime().call(AbiRequest::WatchRegister { pattern }) {
        AbiResponse::WatchRegistered { watch_id } => Some(WatchHandle { id: watch_id }),
        _ => None,
    }
}

pub fn poll_watch(handle: &WatchHandle) -> Vec<GraphChange> {
    runtime::ensure_kernel_runtime();
    match runtime::runtime().call(AbiRequest::WatchPoll {
        watch_id: handle.id,
        max_events: None,
    }) {
        AbiResponse::WatchEvents { events } => events.changes,
        _ => Vec::new(),
    }
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

pub fn log_args(args: fmt::Arguments) {
    struct LogWriter;
    impl fmt::Write for LogWriter {
        fn write_str(&mut self, s: &str) -> fmt::Result {
            unsafe {
                sys::syscall(sys::SYSCALL_LOG, s.as_ptr() as u64, s.len() as u64, 0, 0);
            }
            Ok(())
        }
    }
    let _ = fmt::write(&mut LogWriter, args);
}
