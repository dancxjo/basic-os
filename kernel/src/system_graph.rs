use crate::thing::{Fact, Predicate, Space, Uri};
use alloc::boxed::Box;
use alloc::collections::BTreeMap;
use alloc::string::String;
use alloc::vec::Vec;

pub struct SystemGraph {
    spaces: BTreeMap<&'static str, Box<dyn Space>>, // keyed by protocol (e.g., "os", "file")
}

impl SystemGraph {
    pub fn new() -> Self {
        SystemGraph {
            spaces: BTreeMap::new(),
        }
    }

    pub fn register(&mut self, protocol: &'static str, space: Box<dyn Space>) {
        self.spaces.insert(protocol, space);
    }

    fn find_space(&self, uri: &Uri) -> Option<&dyn Space> {
        let scheme = uri.0.splitn(2, "://").next()?;
        self.spaces.get(scheme).map(|b| &**b)
    }

    fn find_space_mut(&mut self, uri: &Uri) -> Option<&mut dyn Space> {
        let scheme = uri.0.splitn(2, "://").next()?;
        if let Some(b) = self.spaces.get_mut(scheme) {
            Some(&mut **b)
        } else {
            None
        }
    }
}

impl Space for SystemGraph {
    fn protocols(&self) -> &[&'static str] {
        const EMPTY: &[&'static str] = &[];
        EMPTY // TODO: gather from registered spaces
    }

    fn content(&self, uri: &Uri) -> Option<&[u8]> {
        self.find_space(uri).and_then(|s| s.content(uri))
    }

    fn neighbors(&self, from: &Uri, pred: &Predicate) -> Vec<Uri> {
        self.find_space(from)
            .map(|s| s.neighbors(from, pred))
            .unwrap_or_else(Vec::new)
    }

    fn kind(&self, uri: &Uri) -> Option<Uri> {
        self.find_space(uri).and_then(|s| s.kind(uri))
    }

    fn has_fact(&self, from: &Uri, pred: &Predicate, to: &Uri) -> bool {
        self.find_space(from)
            .map(|s| s.has_fact(from, pred, to))
            .unwrap_or(false)
    }

    fn write_content(&mut self, uri: &Uri, data: &[u8]) -> Result<(), String> {
        self.find_space_mut(uri)
            .map(|s| s.write_content(uri, data))
            .unwrap_or_else(|| Err("protocol not found".into()))
    }

    fn add_fact(&mut self, from: &Uri, pred: Predicate, to: &Uri) -> Result<(), String> {
        self.find_space_mut(from)
            .map(|s| s.add_fact(from, pred, to))
            .unwrap_or_else(|| Err("protocol not found".into()))
    }

    fn assert(&mut self, fact: Fact) -> Result<(), String> {
        self.add_fact(&fact.this, fact.predicate, &fact.that)
    }

    fn remove_fact(&mut self, from: &Uri, pred: &Predicate, to: &Uri) -> Result<(), String> {
        self.find_space_mut(from)
            .map(|s| s.remove_fact(from, pred, to))
            .unwrap_or_else(|| Err("protocol not found".into()))
    }

    fn facts(&self, from: &Uri) -> Vec<Fact> {
        self.find_space(from)
            .map(|s| s.facts(from))
            .unwrap_or_else(Vec::new)
    }
}
