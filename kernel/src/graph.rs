// src/graph.rs
use crate::thing::Thing;
use alloc::vec::Vec;

pub struct Graph {
    pub things: Vec<Thing<&'static [u8]>>,
    pub facts: Vec<Fact>,
    pub kinds: Vec<Kind>,
    pub predicates: Vec<Predicate>,
}

pub struct Fact {
    pub this: usize,
    pub that: usize,
    pub predicate: usize,
}

pub struct Kind {
    pub name: &'static str,
    pub description: &'static str,
}

pub struct Predicate {
    pub name: &'static str,
    pub this_kind: &'static str,
    pub that_kind: &'static str,
}

impl Graph {
    pub fn create_thing(
        &mut self,
        name: &'static str,
        kind: &'static str,
        blob: &'static [u8],
    ) -> usize {
        let thing = Thing {
            kind: Some(kind),
            name: Some(name),
            data: blob,
            fields: Vec::new(),
        };
        self.things.push(thing);
        self.things.len() - 1
    }

    pub fn link(&mut self, this: usize, that: usize, predicate: &'static str) {
        let pred_idx = self
            .predicates
            .iter()
            .position(|p| p.name == predicate)
            .expect("Predicate must exist before linking!");
        self.facts.push(Fact {
            this,
            that,
            predicate: pred_idx,
        });
    }

    pub fn add_kind(&mut self, name: &'static str, description: &'static str) {
        self.kinds.push(Kind { name, description });
    }

    pub fn add_predicate(
        &mut self,
        name: &'static str,
        this_kind: &'static str,
        that_kind: &'static str,
    ) {
        self.predicates.push(Predicate {
            name,
            this_kind,
            that_kind,
        });
    }
}
