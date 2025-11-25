use crate::serial_println;
use crate::telemetry::canon::Symbol;
use alloc::{collections::BTreeMap, string::String, vec::Vec};
use core::fmt;
use serde::{Deserialize, Serialize};
use spin::Mutex;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum Value {
    Null,
    Bool(bool),
    U64(u64),
    I64(i64),
    Bytes(Vec<u8>),
    Symbol(Symbol),
    Uuid(Uuid),
    Text(String),
    Map(BTreeMap<Symbol, Value>),
    List(Vec<Value>),
}

impl Value {
    pub fn as_map(&self) -> Option<&BTreeMap<Symbol, Value>> {
        match self {
            Value::Map(m) => Some(m),
            _ => None,
        }
    }

    pub fn as_uuid(&self) -> Option<Uuid> {
        match self {
            Value::Uuid(id) => Some(*id),
            _ => None,
        }
    }

    pub fn as_symbol(&self) -> Option<Symbol> {
        match self {
            Value::Symbol(sym) => Some(*sym),
            _ => None,
        }
    }

    pub fn as_u64(&self) -> Option<u64> {
        match self {
            Value::U64(v) => Some(*v),
            _ => None,
        }
    }

    pub fn as_i64(&self) -> Option<i64> {
        match self {
            Value::I64(v) => Some(*v),
            _ => None,
        }
    }

    pub fn as_bool(&self) -> Option<bool> {
        match self {
            Value::Bool(v) => Some(*v),
            _ => None,
        }
    }

    pub fn as_text(&self) -> Option<&str> {
        match self {
            Value::Text(s) => Some(s),
            _ => None,
        }
    }
}

impl fmt::Display for Value {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Value::Null => f.write_str("null"),
            Value::Bool(v) => write!(f, "{}", v),
            Value::U64(v) => write!(f, "{}", v),
            Value::I64(v) => write!(f, "{}", v),
            Value::Bytes(b) => write!(f, "bytes({})", b.len()),
            Value::Symbol(sym) => write!(f, "{}", sym),
            Value::Uuid(id) => write!(f, "{}", id),
            Value::Text(s) => write!(f, "\"{}\"", s),
            Value::Map(m) => {
                f.write_str("{")?;
                let mut first = true;
                for (k, v) in m.iter() {
                    if !first {
                        f.write_str(", ")?;
                    }
                    first = false;
                    write!(f, "{}: {}", k, v)?;
                }
                f.write_str("}")
            }
            Value::List(items) => {
                f.write_str("[")?;
                let mut first = true;
                for v in items {
                    if !first {
                        f.write_str(", ")?;
                    }
                    first = false;
                    write!(f, "{}", v)?;
                }
                f.write_str("]")
            }
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Event {
    pub timestamp: u64,
    pub kind: Symbol,
    pub data: Value,
}

impl Event {
    pub fn new(kind: Symbol, data: Value) -> Self {
        Self {
            timestamp: timestamp_now(),
            kind,
            data,
        }
    }

    pub fn with_timestamp(timestamp: u64, kind: Symbol, data: Value) -> Self {
        Self {
            timestamp,
            kind,
            data,
        }
    }
}

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
    crate::telemetry::graph::with_store(|store| store.apply_event(&event));
    true
}

/// Convenience helper to wrap data and fill in the timestamp automatically.
pub fn emit_data(kind: Symbol, data: Value) -> bool {
    emit(Event::new(kind, data))
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
