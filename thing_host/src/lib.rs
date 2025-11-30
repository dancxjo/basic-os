use parking_lot::Mutex;
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use thing_abi::{
    AbiRequest, AbiResponse, GraphChange, GraphEdge, GraphFiatRequest, GraphLinkRequest,
    GraphPropsGetRequest, GraphPropsRequest, GraphThing, GraphWatchBatch, NodePattern,
    ThingRuntime, WatchId,
};
use uuid::Uuid;

mod store;
pub use store::{init_graph_store, GraphConfig, GraphStore};

pub struct HostRuntime {
    store: Arc<dyn GraphStore>,
    rt: tokio::runtime::Runtime,
    watchers: Mutex<HashMap<WatchId, WatchState>>,
    next_watch: AtomicU64,
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
        let store = rt.block_on(init_graph_store(config));

        Self {
            store,
            rt,
            watchers: Mutex::new(HashMap::new()),
            next_watch: AtomicU64::new(1),
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
                rel,
                to,
                props,
            } => {
                let req = GraphLinkRequest {
                    id,
                    kind: rel,
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
            AbiRequest::Get { id } => match self.rt.block_on(self.store.get(id)) {
                Ok(thing) => AbiResponse::Get { thing },
                Err(e) => AbiResponse::Error {
                    message: e.to_string(),
                },
            },
            AbiRequest::Query { pattern } => match self.rt.block_on(self.store.query(pattern)) {
                Ok(things) => AbiResponse::Query { things },
                Err(e) => AbiResponse::Error {
                    message: e.to_string(),
                },
            },
            AbiRequest::FindByKind { kind, cursor } => {
                match self.rt.block_on(self.store.find_by_kind(kind, cursor)) {
                    Ok(things) => AbiResponse::Find {
                        things,
                        next_cursor: None,
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
            AbiRequest::WatchPoll { watch_id, .. } => {
                let mut watchers = self.watchers.lock();
                if let Some(watcher) = watchers.get_mut(&watch_id) {
                    let changes = core::mem::take(&mut watcher.queue);
                    let latest_revision = watcher.last_revision;
                    AbiResponse::WatchEvents {
                        events: GraphWatchBatch {
                            from_revision: 0,
                            latest_revision,
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
    if !pattern.labels.is_empty() && !pattern.labels.iter().any(|label| *label == edge.pred) {
        return false;
    }
    pattern.props.is_empty()
}

fn match_change(change: &GraphChange, pattern: &NodePattern) -> bool {
    match change {
        GraphChange::Thing(thing) => pattern_matches(thing, pattern),
        GraphChange::Edge(edge) => edge_matches(edge, pattern),
    }
}
