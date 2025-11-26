//! Simple watch/subscribe manager for userland apps and drivers.
//! Apps register filters over journal or graph data, and the manager fans out
//! matches into per-app inboxes that can be drained before each tick.

use alloc::collections::{BTreeMap, VecDeque};
use alloc::vec::Vec;

use crate::canon;
use crate::graph::{graph_snapshot, Event, GraphSnapshot};
use crate::{Symbol, Value};
use spin::Mutex;
use uuid::Uuid;

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum WatchSource {
    Journal,
    Graph,
}

#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
pub struct WatchId(u32);

#[derive(Clone, Debug)]
pub struct EventFilter {
    pub kind: Option<Symbol>,
    pub src: Option<Uuid>,
    pub dst: Option<Uuid>,
    pub field_eq: &'static [(Symbol, Value)],
}

#[derive(Clone, Debug)]
pub struct ThingFilter {
    pub kind: Option<Symbol>,
    pub id: Option<Uuid>,
    pub predicate: Option<Symbol>,
}

#[derive(Clone, Debug)]
pub enum AppEvent {
    Journal { watch: WatchId, event: Event },
    Graph { watch: WatchId, thing_id: Uuid },
}

#[derive(Default)]
struct AppInbox {
    queue: VecDeque<AppEvent>,
}

#[derive(Clone)]
struct WatchRegistration {
    app_id: usize,
    watch_id: WatchId,
    source: WatchSource,
    event_filter: Option<EventFilter>,
    thing_filter: Option<ThingFilter>,
}

#[derive(Default)]
struct WatchManager {
    next_app: usize,
    next_watch: u32,
    registrations: Vec<WatchRegistration>,
    inboxes: BTreeMap<usize, AppInbox>,
    thing_revisions: BTreeMap<Uuid, u64>,
    edge_revisions: BTreeMap<(Uuid, Symbol, Uuid), u64>,
    last_journal_ts: u64,
}

impl WatchManager {
    fn alloc_app(&mut self) -> usize {
        let id = self.next_app;
        self.next_app = self.next_app.wrapping_add(1);
        self.inboxes.entry(id).or_insert_with(AppInbox::default);
        id
    }

    fn alloc_watch(&mut self) -> WatchId {
        let id = WatchId(self.next_watch);
        self.next_watch = self.next_watch.wrapping_add(1);
        id
    }

    fn register_journal(&mut self, app_id: usize, filter: EventFilter) -> WatchId {
        let watch_id = self.alloc_watch();
        self.registrations.push(WatchRegistration {
            app_id,
            watch_id,
            source: WatchSource::Journal,
            event_filter: Some(filter),
            thing_filter: None,
        });
        watch_id
    }

    fn register_graph(&mut self, app_id: usize, filter: ThingFilter) -> WatchId {
        let watch_id = self.alloc_watch();
        self.registrations.push(WatchRegistration {
            app_id,
            watch_id,
            source: WatchSource::Graph,
            event_filter: None,
            thing_filter: Some(filter),
        });
        watch_id
    }

    fn enqueue(&mut self, app_id: usize, event: AppEvent) {
        let inbox = self.inboxes.entry(app_id).or_insert_with(AppInbox::default);
        inbox.queue.push_back(event);
    }

    fn poll_watch(&mut self, app_id: usize, watch: WatchId) -> Option<AppEvent> {
        let inbox = self.inboxes.get_mut(&app_id)?;
        if let Some(pos) = inbox.queue.iter().position(|ev| match ev {
            AppEvent::Journal { watch: w, .. } | AppEvent::Graph { watch: w, .. } => *w == watch,
        }) {
            return inbox.queue.remove(pos);
        }
        None
    }

    fn next_event(&mut self, app_id: usize) -> Option<AppEvent> {
        self.inboxes.get_mut(&app_id)?.queue.pop_front()
    }

    fn ingest_journal(&mut self, events: &[Event]) {
        for evt in events {
            if evt.timestamp <= self.last_journal_ts {
                continue;
            }
            self.last_journal_ts = evt.timestamp;
            for reg in self
                .registrations
                .iter()
                .filter(|r| r.source == WatchSource::Journal)
            {
                let Some(filter) = &reg.event_filter else {
                    continue;
                };
                if !matches_event(filter, evt) {
                    continue;
                }
                self.enqueue(
                    reg.app_id,
                    AppEvent::Journal {
                        watch: reg.watch_id,
                        event: evt.clone(),
                    },
                );
            }
        }
    }

    fn process_graph(&mut self) {
        let snapshot = match graph_snapshot() {
            Some(s) => s,
            None => return,
        };
        self.process_graph_snapshot(&snapshot);
    }

    fn process_graph_snapshot(&mut self, snapshot: &GraphSnapshot) {
        for thing in snapshot.things.iter() {
            let current_rev = *self.thing_revisions.get(&thing.id).unwrap_or(&0);
            if thing.revision > current_rev {
                self.enqueue_graph_change(thing.id, thing.kind, None);
                self.thing_revisions.insert(thing.id, thing.revision);
            } else if current_rev == 0 {
                self.thing_revisions.insert(thing.id, thing.revision);
            }
        }

        for edge in snapshot.edges.iter() {
            let key = (edge.src, edge.pred, edge.dst);
            let current_rev = *self.edge_revisions.get(&key).unwrap_or(&0);
            if edge.revision > current_rev {
                self.enqueue_graph_change(edge.src, edge.pred, Some(edge.pred));
                self.edge_revisions.insert(key, edge.revision);
            } else if current_rev == 0 {
                self.edge_revisions.insert(key, edge.revision);
            }
        }
    }

    fn enqueue_graph_change(&mut self, thing_id: Uuid, kind: Symbol, predicate: Option<Symbol>) {
        for reg in self
            .registrations
            .iter()
            .filter(|r| r.source == WatchSource::Graph)
        {
            let Some(filter) = &reg.thing_filter else {
                continue;
            };
            if !matches_thing(filter, thing_id, kind, predicate) {
                continue;
            }
            self.enqueue(
                reg.app_id,
                AppEvent::Graph {
                    watch: reg.watch_id,
                    thing_id,
                },
            );
        }
    }
}

fn matches_event(filter: &EventFilter, evt: &Event) -> bool {
    if let Some(kind) = filter.kind {
        if kind != evt.kind {
            return false;
        }
    }

    let data = match evt.data.as_map() {
        Some(m) => m,
        None => return filter.src.is_none() && filter.dst.is_none() && filter.field_eq.is_empty(),
    };

    if let Some(src) = filter.src {
        if data.get(&canon::SRC).and_then(Value::as_uuid) != Some(src) {
            return false;
        }
    }

    if let Some(dst) = filter.dst {
        if data.get(&canon::DST).and_then(Value::as_uuid) != Some(dst) {
            return false;
        }
    }

    for (key, val) in filter.field_eq.iter() {
        let Some(actual) = data.get(key) else {
            return false;
        };
        if actual != val {
            return false;
        }
    }

    true
}

fn matches_thing(
    filter: &ThingFilter,
    thing_id: Uuid,
    kind: Symbol,
    predicate: Option<Symbol>,
) -> bool {
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

    if let Some(pred) = filter.predicate {
        if Some(pred) != predicate {
            return false;
        }
    }

    true
}

static MANAGER: Mutex<WatchManager> = Mutex::new(WatchManager::default());

pub fn register_app() -> usize {
    MANAGER.lock().alloc_app()
}

pub fn watch_journal(app_id: usize, filter: EventFilter) -> WatchId {
    MANAGER.lock().register_journal(app_id, filter)
}

pub fn watch_graph(app_id: usize, filter: ThingFilter) -> WatchId {
    MANAGER.lock().register_graph(app_id, filter)
}

pub fn poll_watch(app_id: usize, watch: WatchId) -> Option<AppEvent> {
    MANAGER.lock().poll_watch(app_id, watch)
}

pub fn next_event(app_id: usize) -> Option<AppEvent> {
    MANAGER.lock().next_event(app_id)
}

pub fn ingest_journal(events: &[Event]) {
    MANAGER.lock().ingest_journal(events);
}

pub fn process_graph() {
    MANAGER.lock().process_graph();
}

pub struct WatchDrain {
    app_id: usize,
}

impl Iterator for WatchDrain {
    type Item = AppEvent;

    fn next(&mut self) -> Option<Self::Item> {
        next_event(self.app_id)
    }
}

pub fn drain_app(app_id: usize) -> WatchDrain {
    WatchDrain { app_id }
}
