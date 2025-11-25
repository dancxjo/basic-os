use crate::serial_println;
use crate::telemetry::canon::sym_name;
use alloc::vec::Vec;
use core::fmt;
use spin::Mutex;

#[derive(Copy, Clone)]
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

const JOURNAL_CAPACITY: usize = 1024;

struct Journal {
    entries: Vec<Proposition>,
}

impl Journal {
    fn new() -> Self {
        Self {
            entries: Vec::new(),
        }
    }

    fn push(&mut self, p: Proposition) -> bool {
        if self.entries.len() >= JOURNAL_CAPACITY {
            return false;
        }
        self.entries.push(p);
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
    let mut j = JOURNAL.lock();
    j.push(Proposition::new(subject, predicate, object))
}

/// Dump the journal as hex codes.
pub fn dump() {
    let j = JOURNAL.lock();
    for entry in &j.entries {
        serial_println!(
            "{:#06x} {:#06x} {:#06x}",
            entry.subject,
            entry.predicate,
            entry.object
        );
    }
}

/// Dump the journal using symbolic names when possible.
pub fn dump_pretty() {
    let j = JOURNAL.lock();
    for entry in &j.entries {
        serial_println!(
            "{} {} {}",
            fmt_sym(entry.subject),
            fmt_sym(entry.predicate),
            fmt_sym(entry.object)
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
pub fn replay(mut f: impl FnMut(&Proposition)) {
    let snapshot = snapshot();
    for entry in &snapshot {
        f(entry);
    }
}

/// Return a snapshot of the current journal.
pub fn snapshot() -> Vec<Proposition> {
    let j = JOURNAL.lock();
    j.entries.clone()
}
