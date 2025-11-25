use crate::serial_println;
use crate::telemetry::canon::sym_name;
use alloc::vec::Vec;
use core::fmt;
use serde::{Deserialize, Serialize};
use spin::Mutex;

#[derive(Copy, Clone, Serialize, Deserialize)]
#[repr(C, packed)]
pub struct Proposition {
    pub subject: u16,
    pub predicate: u16,
    pub object: u16,
}

impl Proposition {
    pub const fn new(subject: u16, predicate: u16, object: u16) -> Self {
        Self {
            subject,
            predicate,
            object,
        }
    }
}

impl fmt::Debug for Proposition {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Proposition")
            .field("subject", &self.subject)
            .field("predicate", &self.predicate)
            .field("object", &self.object)
            .finish()
    }
}

/// Serialized event entry in the journal.
#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct Event {
    pub proposition: Proposition,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub payload: Option<Vec<u8>>,
}

impl Event {
    pub fn new(proposition: Proposition) -> Self {
        Self {
            proposition,
            payload: None,
        }
    }

    pub fn with_payload(proposition: Proposition, payload: Vec<u8>) -> Self {
        Self {
            proposition,
            payload: Some(payload),
        }
    }
}

const JOURNAL_CAPACITY: usize = 1024;

struct Journal {
    entries: Vec<Event>,
}

impl Journal {
    fn new() -> Self {
        Self {
            entries: Vec::new(),
        }
    }

    fn push(&mut self, e: Event) -> bool {
        if self.entries.len() >= JOURNAL_CAPACITY {
            return false;
        }
        self.entries.push(e);
        true
    }
}

static JOURNAL: Mutex<Journal> = Mutex::new(Journal::new());

pub fn init() {
    let mut j = JOURNAL.lock();
    j.entries.clear();
}

/// Record a proposition; returns false if the journal is full.
pub fn record(subject: u16, predicate: u16, object: u16) -> bool {
    record_event(Event::new(Proposition::new(subject, predicate, object)))
}

/// Record a full event with optional payload.
pub fn record_event(event: Event) -> bool {
    let mut j = JOURNAL.lock();
    j.push(event)
}

/// Dump the journal as hex codes.
pub fn dump() {
    let j = JOURNAL.lock();
    for entry in &j.entries {
        let p = entry.proposition;
        serial_println!("{:#06x} {:#06x} {:#06x}", p.subject, p.predicate, p.object);
    }
}

/// Dump the journal using symbolic names when possible.
pub fn dump_pretty() {
    let j = JOURNAL.lock();
    for entry in &j.entries {
        let p = entry.proposition;
        serial_println!(
            "{} {} {}",
            fmt_sym(p.subject),
            fmt_sym(p.predicate),
            fmt_sym(p.object)
        );
    }
}

fn fmt_sym(code: u16) -> SymDisplay {
    SymDisplay { code }
}

struct SymDisplay {
    code: u16,
}

impl fmt::Display for SymDisplay {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let name = sym_name(self.code);
        if name.is_empty() {
            write!(f, "0x{:04x}", self.code)
        } else {
            f.write_str(name)
        }
    }
}

/// Replay journal entries through a visitor.
pub fn replay(mut f: impl FnMut(&Event)) {
    let snapshot = snapshot();
    for entry in &snapshot {
        f(entry);
    }
}

/// Return a snapshot of the current journal.
pub fn snapshot() -> Vec<Event> {
    let j = JOURNAL.lock();
    j.entries.clone()
}

/// Export the journal to a postcard-serialized buffer.
pub fn export_bytes() -> Option<Vec<u8>> {
    let snapshot = snapshot();
    postcard::to_allocvec(&snapshot).ok()
}

/// Import a serialized journal buffer, appending entries if capacity allows.
pub fn import_bytes(buf: &[u8]) -> Result<(), postcard::Error> {
    let events: Vec<Event> = postcard::from_bytes(buf)?;
    let mut j = JOURNAL.lock();
    for e in events {
        if !j.push(e) {
            break;
        }
    }
    Ok(())
}
