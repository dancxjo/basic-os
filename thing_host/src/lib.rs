use parking_lot::Mutex;
use std::collections::{HashMap, VecDeque};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use thing_abi::{
    AbiRequest, AbiResponse, GraphChange, GraphEdge, GraphFiatRequest, GraphGetRequest,
    GraphLinkRequest, GraphThing, GraphWatchBatch, NodePattern, ThingRuntime, WatchId,
};
mod store;
mod symbols;
pub use store::{init_graph_store, GraphBackend, GraphConfig, GraphStore};

pub struct HostRuntime {
    store: Arc<dyn GraphStore>,
    rt: tokio::runtime::Runtime,
    watchers: Mutex<HashMap<WatchId, WatchState>>,
    next_watch: AtomicU64,
    keyboard_fifo: Mutex<VecDeque<u8>>,
    mouse_fifo: Mutex<VecDeque<u8>>,
    open_devices: Mutex<HashMap<u64, DeviceState>>,
    next_handle: AtomicU64,
}

struct DeviceState {
    kind: u32,
    index: usize,
}

struct WatchState {
    pattern: NodePattern,
    queue: Vec<GraphChange>,
    last_revision: u64,
}

impl HostRuntime {
    pub fn new() -> Self {
        let config = GraphConfig::from_env();
        let rt = tokio::runtime::Runtime::new().unwrap();
        let (store, backend) = rt.block_on(init_graph_store(&config));
        match backend {
            GraphBackend::InMemory => {
                println!("Graph backend: InMemory");
            }
            #[cfg(feature = "neo4j")]
            GraphBackend::Neo4j => {
                println!("Graph backend: Neo4j ({})", config.neo4j.uri);
            }
        }

        Self {
            store,
            rt,
            watchers: Mutex::new(HashMap::new()),
            next_watch: AtomicU64::new(1),
            keyboard_fifo: Mutex::new(VecDeque::new()),
            mouse_fifo: Mutex::new(VecDeque::new()),
            open_devices: Mutex::new(HashMap::new()),
            next_handle: AtomicU64::new(1),
        }
    }

    pub fn push_scancode(&self, code: u8) {
        self.keyboard_fifo.lock().push_back(code);
    }

    pub fn push_mouse_packet(&self, packet: &[u8]) {
        let mut fifo = self.mouse_fifo.lock();
        for &b in packet {
            fifo.push_back(b);
        }
    }

    fn push_change(&self, change: GraphChange) {
        let mut watchers = self.watchers.lock();
        for watcher in watchers.values_mut() {
            if match_change(&change, &watcher.pattern) {
                watcher.queue.push(change.clone());
                watcher.last_revision = change.revision();
            }
        }
    }
}

impl ThingRuntime for HostRuntime {
    fn call(&self, req: AbiRequest) -> AbiResponse {
        match req {
            AbiRequest::Fiat {
                id,
                kind,
                mut labels,
                fields,
            } => {
                if labels.is_empty() {
                    labels.push(kind);
                }
                let req = GraphFiatRequest {
                    id,
                    kind,
                    labels,
                    fields,
                };
                match self.rt.block_on(self.store.fiat(req)) {
                    Ok(thing) => {
                        self.push_change(GraphChange::Thing(thing.clone()));
                        AbiResponse::Fiat { thing }
                    }
                    Err(e) => AbiResponse::Error {
                        message: e.to_string(),
                    },
                }
            }
            AbiRequest::Link {
                id,
                from,
                pred,
                to,
                props,
            } => {
                let req = GraphLinkRequest {
                    id,
                    pred,
                    from,
                    to,
                    props,
                };
                match self.rt.block_on(self.store.link(req)) {
                    Ok(edge) => {
                        self.push_change(GraphChange::Edge(edge.clone()));
                        AbiResponse::Link { edge: Some(edge) }
                    }
                    Err(e) => AbiResponse::Error {
                        message: e.to_string(),
                    },
                }
            }
            AbiRequest::Get { id } => {
                match self.rt.block_on(self.store.get(GraphGetRequest::Thing(id))) {
                    Ok(things) => AbiResponse::Get {
                        thing: things.into_iter().next(),
                    },
                    Err(e) => AbiResponse::Error {
                        message: e.to_string(),
                    },
                }
            }
            AbiRequest::Query { pattern } => match self
                .rt
                .block_on(self.store.get(GraphGetRequest::Pattern(pattern)))
            {
                Ok(things) => AbiResponse::Query { things },
                Err(e) => AbiResponse::Error {
                    message: e.to_string(),
                },
            },
            AbiRequest::FindByKind { kind, cursor } => {
                match self.rt.block_on(self.store.find_by_kind(kind, cursor)) {
                    Ok((things, next_cursor)) => AbiResponse::Find {
                        things,
                        next_cursor,
                    },
                    Err(e) => AbiResponse::Error {
                        message: e.to_string(),
                    },
                }
            }
            AbiRequest::WatchRegister { pattern } => {
                let id = self.next_watch.fetch_add(1, Ordering::SeqCst);
                self.watchers.lock().insert(
                    id,
                    WatchState {
                        pattern,
                        queue: Vec::new(),
                        last_revision: 0,
                    },
                );
                AbiResponse::WatchRegistered { watch_id: id }
            }
            AbiRequest::WatchUnregister { watch_id } => {
                self.watchers.lock().remove(&watch_id);
                AbiResponse::WatchUnregistered
            }
            AbiRequest::WatchPoll {
                watch_id,
                max_events,
            } => {
                let snapshot = {
                    let watchers = self.watchers.lock();
                    watchers
                        .get(&watch_id)
                        .map(|w| (w.pattern.clone(), w.last_revision))
                };

                if let Some((pattern, last_revision)) = snapshot {
                    let fetched = match self
                        .rt
                        .block_on(self.store.get(GraphGetRequest::Pattern(pattern)))
                    {
                        Ok(things) => things
                            .into_iter()
                            .filter(|thing| thing.revision > last_revision)
                            .map(GraphChange::Thing)
                            .collect::<Vec<_>>(),
                        Err(e) => {
                            eprintln!("watch poll failed to load graph state: {e}");
                            Vec::new()
                        }
                    };

                    let mut watchers = self.watchers.lock();
                    if let Some(watcher) = watchers.get_mut(&watch_id) {
                        watcher.queue.extend(fetched);
                        watcher.queue.sort_by_key(|change| change.revision());

                        let latest_seen = watcher
                            .queue
                            .iter()
                            .map(GraphChange::revision)
                            .chain(Some(watcher.last_revision))
                            .max()
                            .unwrap_or(watcher.last_revision);

                        let drain_count = max_events
                            .map(|count| count as usize)
                            .unwrap_or_else(|| watcher.queue.len())
                            .min(watcher.queue.len());
                        let changes: Vec<_> = watcher.queue.drain(0..drain_count).collect();

                        watcher.last_revision = latest_seen;

                        AbiResponse::WatchEvents {
                            events: GraphWatchBatch {
                                from_revision: last_revision,
                                latest_revision: latest_seen,
                                changes,
                            },
                        }
                    } else {
                        AbiResponse::WatchEvents {
                            events: GraphWatchBatch {
                                from_revision: 0,
                                latest_revision: 0,
                                changes: Vec::new(),
                            },
                        }
                    }
                } else {
                    AbiResponse::WatchEvents {
                        events: GraphWatchBatch {
                            from_revision: 0,
                            latest_revision: 0,
                            changes: Vec::new(),
                        },
                    }
                }
            }
            AbiRequest::PropsGet { request } => {
                match self.rt.block_on(self.store.props_get(request)) {
                    Ok(props) => AbiResponse::Props { props },
                    Err(e) => AbiResponse::Error {
                        message: e.to_string(),
                    },
                }
            }
            AbiRequest::PropsSet { request } => {
                match self.rt.block_on(self.store.props_set(request)) {
                    Ok(thing) => {
                        self.push_change(GraphChange::Thing(thing.clone()));
                        AbiResponse::Props {
                            props: thing.fields,
                        }
                    }
                    Err(e) => AbiResponse::Error {
                        message: e.to_string(),
                    },
                }
            }
            AbiRequest::DevOpen { kind, index } => {
                let handle = self.next_handle.fetch_add(1, Ordering::Relaxed);
                self.open_devices
                    .lock()
                    .insert(handle, DeviceState { kind, index });
                AbiResponse::DevOpened {
                    handle: Some(handle),
                }
            }
            AbiRequest::DevRead { handle, len } => {
                let mut data = Vec::new();
                let devices = self.open_devices.lock();
                if let Some(state) = devices.get(&handle) {
                    if state.kind == 1 {
                        // Keyboard
                        let mut fifo = self.keyboard_fifo.lock();
                        for _ in 0..len {
                            if let Some(b) = fifo.pop_front() {
                                data.push(b);
                            } else {
                                break;
                            }
                        }
                    } else if state.kind == 2 {
                        // Mouse
                        let mut fifo = self.mouse_fifo.lock();
                        for _ in 0..len {
                            if let Some(b) = fifo.pop_front() {
                                data.push(b);
                            } else {
                                break;
                            }
                        }
                    }
                }
                AbiResponse::DevRead { data }
            }
            AbiRequest::DevWrite { handle: _, data: _ } => {
                // TODO: Implement serial output etc.
                AbiResponse::DevWritten { len: 0 }
            }
            AbiRequest::IrqBind { device: _, line: _ } => {
                // For now, just return a dummy handle
                AbiResponse::IrqBound { handle: Some(1) }
            }
            AbiRequest::IrqAck { handle: _ } => AbiResponse::IrqAcked,
            AbiRequest::GrantCapability { request: _ } => {
                AbiResponse::CapabilityGranted { granted: true }
            }
        }
    }
}

fn pattern_matches(thing: &GraphThing, pattern: &NodePattern) -> bool {
    for label in &pattern.labels {
        if !thing.labels.contains(label) {
            return false;
        }
    }
    for (key, expected) in &pattern.props {
        match thing.fields.get(key) {
            Some(actual) if actual == expected => {}
            _ => return false,
        }
    }
    true
}

fn edge_matches(edge: &GraphEdge, pattern: &NodePattern) -> bool {
    if !pattern.labels.is_empty() {
        let match_found = pattern.labels.iter().any(|label| {
            if let Some(name) = symbols::symbol_name(*label) {
                name == edge.pred
            } else {
                false
            }
        });
        if !match_found {
            return false;
        }
    }
    pattern.props.is_empty()
}

fn match_change(change: &GraphChange, pattern: &NodePattern) -> bool {
    match change {
        GraphChange::Thing(thing) => pattern_matches(thing, pattern),
        GraphChange::Edge(edge) => edge_matches(edge, pattern),
    }
}
