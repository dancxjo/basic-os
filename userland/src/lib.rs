#![no_std]

extern crate alloc;

pub mod drivers;

use alloc::collections::BTreeMap;
use alloc::string::{String, ToString};
use alloc::vec;
use alloc::vec::Vec;
use core::fmt::{self, Write};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Raw syscall entry point (rax, rdi, rsi, rdx).
#[inline(always)]
pub unsafe fn syscall(rax: u64, rdi: u64, rsi: u64, rdx: u64) -> u64 {
    let ret: u64;
    core::arch::asm!(
        "syscall",
        inlateout("rax") rax => ret,
        in("rdi") rdi,
        in("rsi") rsi,
        in("rdx") rdx,
        options(nostack)
    );
    ret
}

const fn canon(a: u8, b: u8, c: u8) -> u32 {
    ((a as u32) << 16) | ((b as u32) << 8) | (c as u32)
}

pub const SYSCALL_WRITE_PORT: u64 = 0x01;
pub const _SYSCALL_READ_PORT: u64 = 0x02;
pub const SYSCALL_JOURNAL_EMIT: u64 = 0x10;
pub const SYSCALL_JOURNAL_SNAPSHOT: u64 = 0x11;
pub const SYSCALL_GRAPH_SNAPSHOT: u64 = 0x12;

const PORT_CONSOLE_OUT: u64 = 1;
const _PORT_CONSOLE_IN: u64 = 2;
const KIND_WRITE: u64 = canon(b'W', b'R', b'T') as u64;

pub fn putchar(c: u8) {
    unsafe {
        syscall(SYSCALL_WRITE_PORT, PORT_CONSOLE_OUT, c as u64, 0);
    }
}

pub fn journal_emit_raw(kind: u32, payload: &[u8]) -> u64 {
    unsafe {
        syscall(
            SYSCALL_JOURNAL_EMIT,
            kind as u64,
            payload.as_ptr() as u64,
            payload.len() as u64,
        )
    }
}

pub fn journal_snapshot_raw(out: &mut [u8]) -> u64 {
    unsafe {
        syscall(
            SYSCALL_JOURNAL_SNAPSHOT,
            out.as_mut_ptr() as u64,
            out.len() as u64,
            0,
        )
    }
}

pub struct Console;

fn emit_write_event(s: &str) -> bool {
    let res = unsafe {
        syscall(
            SYSCALL_JOURNAL_EMIT,
            KIND_WRITE,
            s.as_ptr() as u64,
            s.len() as u64,
        )
    };
    res == 0
}

impl Write for Console {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        if emit_write_event(s) {
            return Ok(());
        }

        for b in s.bytes() {
            putchar(b);
        }
        Ok(())
    }
}

pub fn print_fmt(args: fmt::Arguments) {
    let _ = Console.write_fmt(args);
}

#[macro_export]
macro_rules! print {
    ($($arg:tt)*) => {
        $crate::print_fmt(core::format_args!($($arg)*));
    };
}

#[macro_export]
macro_rules! println {
    () => {
        $crate::print!("\n")
    };
    ($($arg:tt)*) => {
        $crate::print!("{}\n", core::format_args!($($arg)*));
    };
}

#[derive(Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize, Debug)]
pub struct Symbol(pub u32);

impl Symbol {
    pub const fn new(raw: u32) -> Self {
        Symbol(raw)
    }
}

pub mod canon {
    use super::Symbol;

    pub const fn cc(a: char, b: char) -> Symbol {
        Symbol(((a as u32) << 16) | ((b as u32) << 8))
    }

    pub const fn canon(a: u8, b: u8, c: u8) -> Symbol {
        Symbol(((a as u32) << 16) | ((b as u32) << 8) | (c as u32))
    }

    pub const JOURNAL: Symbol = cc('J', 'N');
    pub const KEYBOARD: Symbol = cc('K', 'B');
    pub const KEY_PRESSED: Symbol = cc('K', 'P');
    pub const MOUSE: Symbol = cc('M', 'S');
    pub const MOUSE_MOVED: Symbol = cc('M', 'V');
    pub const AT: Symbol = cc('@', ' ');
    pub const INIT: Symbol = cc('I', 'N');
    pub const FAIL: Symbol = cc('F', 'L');
    pub const DRIVER: Symbol = cc('D', 'R');
    pub const THING_CREATED: Symbol = cc('T', 'C');
    pub const EDGE_ADDED: Symbol = cc('E', 'D');
    pub const ID: Symbol = canon(b'I', b'D', b' ');
    pub const KIND: Symbol = canon(b'K', b'N', b'D');
    pub const FIELDS: Symbol = canon(b'F', b'L', b'D');
    pub const REVISION: Symbol = canon(b'R', b'V', b'N');
    pub const SRC: Symbol = canon(b'S', b'R', b'C');
    pub const DST: Symbol = canon(b'D', b'S', b'T');
    pub const PREDICATE: Symbol = canon(b'P', b'R', b'D');
    pub const NAME: Symbol = canon(b'N', b'A', b'M');
    pub const STATUS: Symbol = canon(b'S', b'T', b'A');
    pub const TEXT: Symbol = canon(b'T', b'X', b'T');
    pub const TARGET: Symbol = canon(b'T', b'G', b'T');
    pub const STDOUT: Symbol = canon(b'S', b'T', b'D');
    pub const COMPOSITOR: Symbol = canon(b'C', b'M', b'P');
    pub const WINDOW: Symbol = canon(b'W', b'I', b'N');
    pub const PIXMAP: Symbol = canon(b'P', b'X', b'M');
    pub const STREAMS: Symbol = canon(b'S', b'T', b'M');
    pub const COMPOSED_BY: Symbol = canon(b'C', b'M', b'B');
    pub const WINDOW_CREATED: Symbol = canon(b'W', b'C', b'R');
    pub const WINDOW_BUFFER_UPDATED: Symbol = canon(b'W', b'B', b'U');
    pub const FRAME_READY: Symbol = canon(b'F', b'R', b'M');
    pub const DRIVER_INPUT: Symbol = canon(b'I', b'N', b'P');
    pub const DRIVER_DISPLAY: Symbol = canon(b'D', b'S', b'P');
    pub const DRIVER_STORAGE: Symbol = canon(b'S', b'T', b'R');
    pub const DRIVER_TIMER: Symbol = canon(b'T', b'M', b'R');
    pub const DRIVER_OTHER: Symbol = canon(b'O', b'T', b'H');
}

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
    pub fn text(s: impl Into<String>) -> Self {
        Value::Text(s.into())
    }

    pub fn symbol(sym: Symbol) -> Self {
        Value::Symbol(sym)
    }

    pub fn uuid(id: Uuid) -> Self {
        Value::Uuid(id)
    }

    pub fn map(map: BTreeMap<Symbol, Value>) -> Self {
        Value::Map(map)
    }

    pub fn as_uuid(&self) -> Option<Uuid> {
        match self {
            Value::Uuid(id) => Some(*id),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Event {
    pub timestamp: u64,
    pub kind: Symbol,
    pub data: Value,
}

pub fn map() -> BTreeMap<Symbol, Value> {
    BTreeMap::new()
}

fn emit(kind: Symbol, data: Value) {
    if let Ok(buf) = postcard::to_allocvec(&data) {
        let _ = journal_emit_raw(kind.0, &buf);
    }
}

pub fn emit_thing_created(id: Uuid, kind: Symbol, revision: u64, fields: BTreeMap<Symbol, Value>) {
    let mut data = map();
    data.insert(canon::ID, Value::Uuid(id));
    data.insert(canon::KIND, Value::Symbol(kind));
    data.insert(canon::REVISION, Value::U64(revision));
    data.insert(canon::FIELDS, Value::Map(fields));
    emit(canon::THING_CREATED, Value::Map(data));
}

pub fn emit_edge_added(src: Uuid, pred: Symbol, dst: Uuid, revision: u64) {
    let mut data = map();
    data.insert(canon::SRC, Value::Uuid(src));
    data.insert(canon::DST, Value::Uuid(dst));
    data.insert(canon::PREDICATE, Value::Symbol(pred));
    data.insert(canon::REVISION, Value::U64(revision));
    emit(canon::EDGE_ADDED, Value::Map(data));
}

pub fn emit_frame_ready(compositor: Uuid, framebuffer: Uuid, pixmap: Uuid, text: &[u8]) {
    let mut payload = map();
    payload.insert(canon::SRC, Value::Uuid(compositor));
    payload.insert(canon::DST, Value::Uuid(framebuffer));
    payload.insert(canon::TARGET, Value::Uuid(pixmap));
    payload.insert(canon::TEXT, Value::Bytes(text.to_vec()));
    emit(canon::FRAME_READY, Value::Map(payload));
}

pub fn emit_window_buffer_updated(window: Uuid, pixmap: Uuid, revision: u64, text: &[u8]) {
    let mut payload = map();
    payload.insert(canon::SRC, Value::Uuid(window));
    payload.insert(canon::TARGET, Value::Uuid(pixmap));
    payload.insert(canon::REVISION, Value::U64(revision));
    payload.insert(canon::TEXT, Value::Bytes(text.to_vec()));
    emit(canon::WINDOW_BUFFER_UPDATED, Value::Map(payload));
}

pub fn fetch_journal_events() -> Option<Vec<Event>> {
    let mut buf = vec![0u8; 4096];
    let needed = journal_snapshot_raw(&mut buf) as usize;
    if needed == 0 {
        return Some(Vec::new());
    }
    if needed > buf.len() {
        buf.resize(needed, 0);
    }
    let written = journal_snapshot_raw(&mut buf) as usize;
    if written == 0 || written > buf.len() {
        return None;
    }
    postcard::from_bytes::<Vec<Event>>(&buf[..written]).ok()
}

pub fn extract_text(value: &Value) -> Option<String> {
    match value {
        Value::Text(s) => Some(s.clone()),
        Value::Bytes(b) => core::str::from_utf8(b).ok().map(ToString::to_string),
        Value::Map(m) => m.get(&canon::TEXT).and_then(extract_text),
        _ => None,
    }
}
