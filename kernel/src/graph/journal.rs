use crate::graph::canon::Symbol;
use crate::serial_println;
use alloc::vec::Vec;
use spin::Mutex;
use thing_abi::{Event, Value};

const JOURNAL_CAPACITY: usize = 1024;

#[derive(Clone)]
struct Journal {
    entries: Vec<Event>,
    capacity: usize,
}

impl Journal {
    fn new(capacity: usize) -> Self {
        Self {
            entries: Vec::new(),
            capacity,
        }
    }

    fn emit(&mut self, e: Event) {
        if self.entries.len() >= self.capacity {
            self.entries.remove(0);
        }
        self.entries.push(e);
    }
}

static JOURNAL: Mutex<Option<Journal>> = Mutex::new(None);

pub fn init() {
    with_journal(|j| j.entries.clear());
}

/// Record an event, evicting the oldest entry when at capacity.
pub fn emit(event: Event) -> bool {
    with_journal(|j| j.emit(event.clone()));
    true
}

/// Convenience helper to wrap data and fill in the timestamp automatically.
pub fn emit_data(kind: Symbol, data: Value) -> bool {
    emit(Event {
        timestamp: timestamp_now(),
        kind,
        data,
    })
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
    with_journal(|j| j.entries.clone())
}

/// Dump the journal using symbolic names when possible.
pub fn dump_pretty() {
    let snapshot = snapshot();
    for entry in &snapshot {
        serial_println!("@{} {} {}", entry.timestamp, entry.kind, entry.data);
    }
}

/// Export the journal to a postcard-serialized buffer.
pub fn export_bytes() -> Option<Vec<u8>> {
    let snapshot = snapshot();
    postcard::to_allocvec(&snapshot).ok()
}

/// Import a serialized journal buffer, appending entries if capacity allows.
pub fn import_bytes(buf: &[u8]) -> Result<(), postcard::Error> {
    let events: Vec<Event> = postcard::from_bytes(buf)?;
    for e in events {
        let _ = emit(e);
    }
    Ok(())
}

fn timestamp_now() -> u64 {
    let clock = unsafe { crate::clock::CLOCK };
    match clock {
        Some(clk) => clk.lock().ticks_since_boot(),
        None => 0,
    }
}

fn with_journal<R>(f: impl FnOnce(&mut Journal) -> R) -> R {
    let mut j = JOURNAL.lock();
    let journal = j.get_or_insert_with(|| Journal::new(JOURNAL_CAPACITY));
    f(journal)
}
