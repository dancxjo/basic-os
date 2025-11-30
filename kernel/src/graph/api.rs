use crate::graph::canon;
use crate::graph::canon::Symbol;
use crate::graph::journal;
use crate::graph::store::Store;
use crate::graph::types::*;
use alloc::collections::BTreeMap;
use alloc::vec;
use alloc::vec::Vec;
use spin::Mutex;
use thing_abi::{Value, WatchQuery};
use uuid::Uuid;
use x86_64::instructions::interrupts;

static STORE: Mutex<Option<Store>> = Mutex::new(None);

pub fn init() {
    journal::init();
    let mut s = STORE.lock();
    if s.is_none() {
        *s = Some(Store::new());
    }
}

pub(crate) fn with_store<R>(f: impl FnOnce(&mut Store) -> R) -> R {
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

pub fn declare_shared_buffer(owner: BundleId, spec: SharedBufferSpec) -> GraphThing {
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

pub fn declare_queue_state(spec: QueueStateSpec) -> GraphThing {
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

pub fn export_thing_bytes(owner: BundleId, id: Uuid) -> Option<Vec<u8>> {
    with_store(|store| {
        if !store.can_read(owner, id) {
            return None;
        }
        let thing = store.latest(&id)?;
        postcard::to_allocvec(&thing).ok()
    })
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
    if let Some(events) = with_store(|store| store.poll_watch(id)) {
        if !events.changes.is_empty() {
            log::info!("Exporting {} events for watch {}", events.changes.len(), id);
            match postcard::to_allocvec(&events) {
                Ok(data) => {
                    log::info!("Exported {} bytes", data.len());
                    return Some(data);
                }
                Err(e) => {
                    log::error!("Serialization error: {:?}", e);
                    return None;
                }
            }
        }
    }
    Some(Vec::new())
}
