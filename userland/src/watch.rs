//! Central watch/subscribe manager shared by the compositor and apps.
//! The manager pulls graph changes from the kernel and fans them out into
//! per-app inboxes based on simple filters.

use alloc::collections::BTreeMap;
use alloc::vec::Vec;

use crate::graph::{self, GraphChange, GraphEdge, GraphThing};
use crate::{canon, Symbol};
use uuid::Uuid;

#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
pub struct WatchId(u32);

#[derive(Clone, Debug)]
pub struct EventFilter {
    pub kind: Option<Symbol>,
    pub src: Option<Uuid>,
    pub dst: Option<Uuid>,
}

#[derive(Clone, Debug)]
pub struct ThingFilter {
    pub kind: Option<Symbol>,
    pub id: Option<Uuid>,
}

#[derive(Clone, Debug)]
pub enum AppEvent {
    Thing { watch: WatchId, thing: GraphThing },
    Edge { watch: WatchId, edge: GraphEdge },
}

pub struct WatchManager {
    next_app: usize,
    next_watch: u32,
    app_watches: BTreeMap<usize, Vec<(WatchId, graph::WatchHandle)>>,
    inboxes: BTreeMap<usize, Vec<AppEvent>>,
}

impl WatchManager {
    pub fn new() -> Self {
        Self {
            next_app: 0,
            next_watch: 0,
            app_watches: BTreeMap::new(),
            inboxes: BTreeMap::new(),
        }
    }

    pub fn register_app(&mut self) -> usize {
        let id = self.next_app;
        self.next_app = self.next_app.wrapping_add(1);
        self.inbox_for(id);
        id
    }

    pub fn register_journal(&mut self, app_id: usize, filter: EventFilter) -> WatchId {
        // Map journal filter to graph watch
        self.register_graph(
            app_id,
            ThingFilter {
                kind: filter.kind,
                id: None,
            },
        )
    }

    pub fn register_graph(&mut self, app_id: usize, filter: ThingFilter) -> WatchId {
        let mut pattern = graph::NodePattern::default();
        if let Some(kind) = filter.kind {
            pattern.labels.push(kind);
        }
        if let Some(id) = filter.id {
            pattern
                .props
                .insert(crate::canon::SRC, graph::Value::Uuid(id));
        }
        self.register_pattern(app_id, pattern)
    }

    pub fn register_pattern(&mut self, app_id: usize, pattern: graph::NodePattern) -> WatchId {
        let watch_id = self.alloc_watch();

        if let Some(handle) = graph::watch_pattern(pattern) {
            self.app_watches
                .entry(app_id)
                .or_default()
                .push((watch_id, handle));
        }

        self.inbox_for(app_id);
        watch_id
    }

    pub fn push_event(&mut self, app_id: usize, ev: AppEvent) {
        let inbox = self.inbox_for(app_id);
        inbox.push(ev);
    }

    pub fn drain_inbox(&mut self, app_id: usize) -> Vec<AppEvent> {
        self.inboxes.insert(app_id, Vec::new()).unwrap_or_default()
    }

    pub fn process_graph(&mut self, app_ids: &[usize]) {
        let mut events_to_push = Vec::new();

        for app_id in app_ids {
            if let Some(watches) = self.app_watches.get_mut(app_id) {
                for (watch_id, handle) in watches {
                    let events = graph::poll_watch(handle);
                    for change in events {
                        let ev = match change {
                            GraphChange::Thing(t) => AppEvent::Thing {
                                watch: *watch_id,
                                thing: t,
                            },
                            GraphChange::Edge(e) => AppEvent::Edge {
                                watch: *watch_id,
                                edge: e,
                            },
                        };
                        events_to_push.push((*app_id, ev));
                    }
                }
            }
        }

        for (app_id, ev) in events_to_push {
            self.push_event(app_id, ev);
        }
    }

    fn alloc_watch(&mut self) -> WatchId {
        let id = WatchId(self.next_watch);
        self.next_watch = self.next_watch.wrapping_add(1);
        id
    }

    fn inbox_for(&mut self, app_id: usize) -> &mut Vec<AppEvent> {
        self.inboxes.entry(app_id).or_insert_with(Vec::new)
    }
}
