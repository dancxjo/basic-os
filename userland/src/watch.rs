//! Central watch/subscribe manager shared by the compositor and apps.
//! The manager pulls batches of journal or graph events and fans them out
//! into per-app inboxes based on simple filters.

use alloc::collections::BTreeMap;
use alloc::vec::Vec;

use crate::canon;
use crate::graph::{graph_snapshot, Event, GraphSnapshot};
use crate::{Symbol, Value};
use uuid::Uuid;

#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
pub struct WatchId(u32);

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum WatchSource {
    Journal,
    Graph,
}

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
    Journal { watch: WatchId, event: Event },
    Graph { watch: WatchId, thing_id: Uuid },
}

#[derive(Clone, Debug)]
pub struct WatchRegistration {
    pub app_id: usize,
    pub watch_id: WatchId,
    pub source: WatchSource,
    pub event_filter: Option<EventFilter>,
    pub thing_filter: Option<ThingFilter>,
}

pub struct WatchManager {
    next_app: usize,
    next_watch: u32,
    regs: Vec<WatchRegistration>,
    inboxes: BTreeMap<usize, Vec<AppEvent>>,
    last_journal_ts: u64,
    thing_revisions: BTreeMap<Uuid, u64>,
}

impl WatchManager {
    pub fn new() -> Self {
        Self {
            next_app: 0,
            next_watch: 0,
            regs: Vec::new(),
            inboxes: BTreeMap::new(),
            last_journal_ts: 0,
            thing_revisions: BTreeMap::new(),
        }
    }

    pub fn register_app(&mut self) -> usize {
        let id = self.next_app;
        self.next_app = self.next_app.wrapping_add(1);
        self.inbox_for(id);
        id
    }

    pub fn register_journal(&mut self, app_id: usize, filter: EventFilter) -> WatchId {
        let watch_id = self.alloc_watch();
        self.regs.push(WatchRegistration {
            app_id,
            watch_id,
            source: WatchSource::Journal,
            event_filter: Some(filter),
            thing_filter: None,
        });
        self.inbox_for(app_id);
        watch_id
    }

    pub fn register_graph(&mut self, app_id: usize, filter: ThingFilter) -> WatchId {
        let watch_id = self.alloc_watch();
        self.regs.push(WatchRegistration {
            app_id,
            watch_id,
            source: WatchSource::Graph,
            event_filter: None,
            thing_filter: Some(filter),
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

    pub fn process_journal_batch(&mut self, app_ids: &[usize], events: &[Event]) {
        for evt in events {
            if evt.timestamp <= self.last_journal_ts {
                continue;
            }
            self.last_journal_ts = evt.timestamp;
            for reg in self
                .regs
                .iter()
                .filter(|r| matches!(r.source, WatchSource::Journal))
            {
                if !app_ids.contains(&reg.app_id) {
                    continue;
                }
                let Some(filter) = &reg.event_filter else {
                    continue;
                };
                if !matches_event(filter, evt) {
                    continue;
                }
                self.push_event(
                    reg.app_id,
                    AppEvent::Journal {
                        watch: reg.watch_id,
                        event: evt.clone(),
                    },
                );
            }
        }
    }

    pub fn process_graph(&mut self, app_ids: &[usize]) {
        let snapshot = match graph_snapshot() {
            Some(s) => s,
            None => return,
        };
        self.process_graph_snapshot(app_ids, &snapshot);
    }

    pub fn process_graph_snapshot(&mut self, app_ids: &[usize], snapshot: &GraphSnapshot) {
        for thing in snapshot.things.iter() {
            let current_rev = *self.thing_revisions.get(&thing.id).unwrap_or(&0);
            if thing.revision > current_rev {
                self.enqueue_graph_change(app_ids, thing.id, thing.kind);
                self.thing_revisions.insert(thing.id, thing.revision);
            } else if current_rev == 0 {
                self.thing_revisions.insert(thing.id, thing.revision);
            }
        }
    }

    fn enqueue_graph_change(&mut self, app_ids: &[usize], thing_id: Uuid, kind: Symbol) {
        for reg in self
            .regs
            .iter()
            .filter(|r| matches!(r.source, WatchSource::Graph))
        {
            if !app_ids.contains(&reg.app_id) {
                continue;
            }
            let Some(filter) = &reg.thing_filter else {
                continue;
            };
            if !matches_thing(filter, thing_id, kind) {
                continue;
            }
            self.push_event(
                reg.app_id,
                AppEvent::Graph {
                    watch: reg.watch_id,
                    thing_id,
                },
            );
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

fn matches_event(filter: &EventFilter, evt: &Event) -> bool {
    if let Some(kind) = filter.kind {
        if kind != evt.kind {
            return false;
        }
    }

    let data = match evt.data.as_map() {
        Some(m) => m,
        None => return filter.src.is_none() && filter.dst.is_none(),
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

    true
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
