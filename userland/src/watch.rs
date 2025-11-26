//! Central watch/subscribe manager shared by the compositor and apps.
//! The manager pulls graph changes from the kernel and fans them out into
//! per-app inboxes based on simple filters.

use alloc::collections::BTreeMap;
use alloc::vec::Vec;

use crate::canon;
use crate::graph::{graph_watch, GraphChange, GraphEdge, GraphThing, GraphWatchBatch};
use crate::Symbol;
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

#[derive(Clone, Debug)]
pub struct WatchRegistration {
    pub app_id: usize,
    pub watch_id: WatchId,
    pub thing_filter: ThingFilter,
}

pub struct WatchManager {
    next_app: usize,
    next_watch: u32,
    regs: Vec<WatchRegistration>,
    inboxes: BTreeMap<usize, Vec<AppEvent>>,
    last_revision: u64,
}

impl WatchManager {
    pub fn new() -> Self {
        Self {
            next_app: 0,
            next_watch: 0,
            regs: Vec::new(),
            inboxes: BTreeMap::new(),
            last_revision: 0,
        }
    }

    pub fn register_app(&mut self) -> usize {
        let id = self.next_app;
        self.next_app = self.next_app.wrapping_add(1);
        self.inbox_for(id);
        id
    }

    pub fn register_journal(&mut self, app_id: usize, filter: EventFilter) -> WatchId {
        self.register_graph(
            app_id,
            ThingFilter {
                kind: filter.kind,
                id: None,
            },
        )
    }

    pub fn register_graph(&mut self, app_id: usize, filter: ThingFilter) -> WatchId {
        let watch_id = self.alloc_watch();
        self.regs.push(WatchRegistration {
            app_id,
            watch_id,
            thing_filter: filter,
        });
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
        let batch = match graph_watch(self.last_revision) {
            Some(b) => b,
            None => return,
        };
        self.process_graph_batch(app_ids, &batch);
        self.last_revision = batch.latest_revision;
    }

    pub fn process_graph_batch(&mut self, app_ids: &[usize], batch: &GraphWatchBatch) {
        for change in batch.changes.iter() {
            match change {
                GraphChange::Thing(thing) => self.enqueue_graph_change(app_ids, thing),
                GraphChange::Edge(edge) => self.enqueue_edge_change(app_ids, edge),
            }
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

    fn enqueue_graph_change(&mut self, app_ids: &[usize], thing: &GraphThing) {
        let mut deliveries = Vec::new();
        for reg in self.regs.iter() {
            if !app_ids.contains(&reg.app_id) {
                continue;
            }
            if !matches_thing(&reg.thing_filter, thing.id, thing.kind) {
                continue;
            }
            deliveries.push((
                reg.app_id,
                AppEvent::Thing {
                    watch: reg.watch_id,
                    thing: thing.clone(),
                },
            ));
        }
        for (app_id, ev) in deliveries {
            self.push_event(app_id, ev);
        }
    }

    fn enqueue_edge_change(&mut self, app_ids: &[usize], edge: &GraphEdge) {
        let mut deliveries = Vec::new();
        for reg in self.regs.iter() {
            if !app_ids.contains(&reg.app_id) {
                continue;
            }
            // Reuse thing filters to match either endpoint.
            if let Some(expected_kind) = reg.thing_filter.kind {
                if expected_kind != canon::EDGE_ADDED {
                    continue;
                }
            }
            if let Some(id) = reg.thing_filter.id {
                if id != edge.src && id != edge.dst {
                    continue;
                }
            }
            deliveries.push((
                reg.app_id,
                AppEvent::Edge {
                    watch: reg.watch_id,
                    edge: edge.clone(),
                },
            ));
        }
        for (app_id, ev) in deliveries {
            self.push_event(app_id, ev);
        }
    }
}

fn matches_thing(filter: &ThingFilter, thing_id: Uuid, kind: Symbol) -> bool {
    if let Some(expected_kind) = filter.kind {
        if expected_kind != kind {
            return false;
        }
    }

    if let Some(expected_id) = filter.id {
        if expected_id != thing_id {
            return false;
        }
    }

    true
}
