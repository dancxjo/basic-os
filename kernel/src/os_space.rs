// os_space.rs — Space implementation for os:// URIs

use crate::thing::{Fact, Predicate, Space, Uri};
use alloc::string::String;
use alloc::vec::Vec;
use core::slice;
use limine::request::{HhdmRequest, MemoryMapRequest};

pub struct OsSpace {
    pub facts: Vec<Fact>,
    pub kernel_memory: &'static [u8],
}

#[used]
pub static HHDM_REQUEST: HhdmRequest = HhdmRequest::new();
#[used]
pub static MEMMAP_REQUEST: MemoryMapRequest = MemoryMapRequest::new();

impl OsSpace {
    pub fn new() -> Self {
        let hhdm = HHDM_REQUEST
            .get_response()
            .expect("No HHDM response")
            .offset();
        let memory_map = MEMMAP_REQUEST.get_response().expect("No memory map");

        let max_phys = memory_map
            .entries()
            .iter()
            .map(|e| e.base + e.length)
            .max()
            .unwrap_or(0);

        let kernel_memory = unsafe { slice::from_raw_parts(hhdm as *const u8, max_phys as usize) };

        let mut facts = Vec::new();
        facts.push(Fact::that(
            Uri("os://kernel".into()),
            &Predicate("is"),
            Uri("kind:kernel".into()),
        ));

        OsSpace {
            facts,
            kernel_memory,
        }
    }
}

impl Space for OsSpace {
    fn protocols(&self) -> &[&'static str] {
        &["os"]
    }

    fn content(&self, uri: &Uri) -> Option<&[u8]> {
        match uri.0.as_str() {
            "os://kernel" => Some(self.kernel_memory),
            _ => None,
        }
    }

    fn neighbors(&self, from: &Uri, pred: &Predicate) -> Vec<Uri> {
        self.facts
            .iter()
            .filter(|f| &f.this == from && &f.predicate == pred)
            .map(|f| f.that.clone())
            .collect()
    }

    fn kind(&self, uri: &Uri) -> Option<Uri> {
        self.facts
            .iter()
            .find(|f| &f.this == uri && f.predicate.0 == "is")
            .map(|f| f.that.clone())
    }

    fn add_fact(&mut self, from: &Uri, pred: Predicate, to: &Uri) -> Result<(), String> {
        self.facts.push(Fact::that(from.clone(), &pred, to.clone()));
        Ok(())
    }

    fn facts(&self, from: &Uri) -> Vec<Fact> {
        self.facts
            .iter()
            .filter(|f| &f.this == from)
            .cloned()
            .collect()
    }
}
