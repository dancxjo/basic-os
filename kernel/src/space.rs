use alloc::boxed::Box;
use alloc::string::{String, ToString};
use alloc::vec;
use alloc::{collections::btree_map::BTreeMap, vec::Vec};
use log::{debug, info};
use uuid::Uuid;

use crate::beat::Beat;
use crate::effects::Effect;
use crate::gui::GUI;
use crate::names::PersonaName;
use crate::thing::{Fact, Kind, Query, Thing};
use crate::verb::Verb;

pub struct Space {
    pub now: u64,
    pub facts: Vec<Fact>,
    pub things: BTreeMap<Uuid, Thing>,
    pub effects: Vec<Effect>,
}

impl Space {
    pub fn new() -> Self {
        Self {
            now: 0,
            facts: vec![],
            things: BTreeMap::new(),
            effects: vec![],
        }
    }

    pub fn dump(&self) {
        for fact in &self.facts {
            let subject = short_hash(fact.subject);

            let verb = self
                .things
                .get(&fact.verb)
                .and_then(|t| t.data.as_any().downcast_ref::<Verb>())
                .map(|v| v.conjugated())
                .unwrap_or_else(|| short_hash(fact.verb));

            let object = short_hash(fact.object);

            info!("{:<16} {:<24} {}", subject, verb, object);
        }
    }

    pub fn commit(&mut self, mut incoming: Vec<Fact>) {
        for fact in incoming.iter_mut() {
            fact.timestamp = self.now;
        }

        let (retractions, affirmations): (Vec<_>, Vec<_>) =
            incoming.into_iter().partition(|f| f.negated);

        // Remove matching affirmed facts
        self.facts
            .retain(|existing| !retractions.iter().any(|r| r.matches(existing)));

        self.facts.extend(affirmations);
    }

    pub fn insert(&mut self, thing: Thing) {
        self.things.insert(thing.id, thing);
    }

    pub fn get(&self, id: Uuid) -> Option<&Thing> {
        self.things.get(&id)
    }

    pub fn get_mut(&mut self, id: Uuid) -> Option<&mut Thing> {
        self.things.get_mut(&id)
    }

    pub fn with<F>(&mut self, id: Uuid, f: F)
    where
        F: FnOnce(&mut Thing),
    {
        if let Some(thing) = self.things.get_mut(&id) {
            f(thing);
        }
    }

    pub fn the<T: Kind + 'static>(&self) -> Query<'_, T> {
        let kind_id = T::uuid();

        let iter = self.things.values().filter_map(move |thing| {
            if thing.kind == kind_id {
                thing.data.as_ref().as_any().downcast_ref::<T>()
            } else {
                None
            }
        });

        Query {
            iter: Box::new(iter),
        }
    }

    pub fn pulse(&mut self) {
        let mut to_commit = vec![];

        // Separate collected facts to avoid borrow checker wrath
        let current_facts = core::mem::take(&mut self.facts);

        debug!("Pulse saw {} facts before filtering.", current_facts.len());

        for fact in current_facts.into_iter() {
            let mut matched = false;

            for effect in &mut self.effects {
                if effect.pattern.matches(&fact) {
                    let new = (effect.handler)(&fact);
                    to_commit.extend(new);
                    matched = true;
                    break; // One match is enough — fact is "consumed"
                }
            }

            // If no effect matched, keep the fact
            if !matched {
                self.facts.push(fact);
            }
        }

        self.commit(to_commit);
    }
}

fn short_hash(uuid: Uuid) -> String {
    let name = PersonaName::from_uuid(uuid);
    name.to_string()
}
