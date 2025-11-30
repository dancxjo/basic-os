use parking_lot::Mutex;
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};

use thing_abi::{
    AbiRequest, AbiResponse, GraphChange, GraphEdge, GraphThing, GraphWatchBatch, NodePattern,
    ThingRuntime, WatchId,
};
use uuid::Uuid;

pub struct HostRuntime {
    things: Mutex<HashMap<Uuid, GraphThing>>,
    edges: Mutex<HashMap<Uuid, GraphEdge>>,
    watchers: Mutex<HashMap<WatchId, WatchState>>,
    next_watch: AtomicU64,
    revision: AtomicU64,
}

struct WatchState {
    pattern: NodePattern,
    queue: Vec<GraphChange>,
    last_revision: u64,
}

impl HostRuntime {
    pub fn new() -> Self {
        Self {
            things: Mutex::new(HashMap::new()),
            edges: Mutex::new(HashMap::new()),
            watchers: Mutex::new(HashMap::new()),
            next_watch: AtomicU64::new(1),
            revision: AtomicU64::new(1),
        }
    }

    fn next_revision(&self) -> u64 {
        self.revision.fetch_add(1, Ordering::SeqCst)
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
                let thing_id = id.unwrap_or_else(|| thing_abi::next_uuid());
                let revision = self.next_revision();
                let thing = GraphThing {
                    id: thing_id,
                    kind,
                    labels: labels.iter().copied().collect(),
                    fields,
                    owner: Uuid::nil(),
                    revision,
                };
                self.things.lock().insert(thing_id, thing.clone());
                self.push_change(GraphChange::Thing(thing.clone()));
                AbiResponse::Fiat { thing }
            }
            AbiRequest::Link {
                id,
                from,
                rel,
                to,
                props,
            } => {
                let edge_id = id.unwrap_or_else(|| thing_abi::next_uuid());
                let revision = self.next_revision();
                let edge = GraphEdge {
                    id: edge_id,
                    src: from,
                    pred: rel,
                    dst: to,
                    props,
                    owner: Uuid::nil(),
                    revision,
                };
                self.edges.lock().insert(edge_id, edge.clone());
                self.push_change(GraphChange::Edge(edge.clone()));
                AbiResponse::Link { edge: Some(edge) }
            }
            AbiRequest::Get { id } => {
                let thing = self.things.lock().get(&id).cloned();
                AbiResponse::Get { thing }
            }
            AbiRequest::Query { pattern } => {
                let things = self
                    .things
                    .lock()
                    .values()
                    .filter(|thing| pattern_matches(thing, &pattern))
                    .cloned()
                    .collect();
                AbiResponse::Query { things }
            }
            AbiRequest::FindByKind { .. } => {
                let things = self.things.lock().values().cloned().collect();
                AbiResponse::Find {
                    things,
                    next_cursor: None,
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
                let thing = self.things.lock().get(&request.node).cloned();
                let props = thing.map(|t| {
                    if request.keys.is_empty() {
                        t.fields
                    } else {
                        request
                            .keys
                            .iter()
                            .filter_map(|k| t.fields.get(k).map(|v| (*k, v.clone())))
                            .collect()
                    }
                });
                props
                    .map(|props| AbiResponse::Props { props })
                    .unwrap_or_else(|| AbiResponse::Error {
                        message: "thing not found".into(),
                    })
            }
            AbiRequest::PropsSet { request } => {
                if let Some(mut thing) = self.things.lock().get_mut(&request.node) {
                    thing.fields.extend(request.props.clone());
                    AbiResponse::Props {
                        props: thing.fields.clone(),
                    }
                } else {
                    AbiResponse::Error {
                        message: "thing not found".into(),
                    }
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
