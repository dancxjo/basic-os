use alloc::vec;
use alloc::vec::Vec;

use crate::graph::{map, Event, Value};
use crate::{canon, sys, Symbol};
use uuid::Uuid;

// Journal snapshots are small; bail out rather than trying to allocate nonsense sizes.
const MAX_JOURNAL_BYTES: usize = 1 << 20; // 1 MiB upper bound

fn emit(kind: Symbol, data: Value) {
    if let Ok(buf) = postcard::to_allocvec(&data) {
        let _ = sys::journal_emit_raw(kind.0, &buf);
    }
}

#[deprecated(note = "use graph::fiat instead")]
pub fn emit_thing_created(id: Uuid, kind: Symbol, revision: u64, fields: crate::graph::Map) {
    let mut data = map();
    data.insert(canon::ID, Value::Uuid(id));
    data.insert(canon::KIND, Value::Symbol(kind));
    data.insert(canon::REVISION, Value::U64(revision));
    data.insert(canon::FIELDS, Value::Map(fields));
    emit(canon::THING_CREATED, Value::Map(data));
}

#[deprecated(note = "use graph::that instead")]
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
    let needed = sys::journal_snapshot_raw(&mut buf) as usize;
    if needed == 0 {
        return Some(Vec::new());
    }
    if needed > MAX_JOURNAL_BYTES {
        return None;
    }
    if needed > buf.len() {
        buf.resize(needed, 0);
    }
    let written = sys::journal_snapshot_raw(&mut buf) as usize;
    if written == 0 || written > buf.len() {
        return None;
    }
    postcard::from_bytes::<Vec<Event>>(&buf[..written]).ok()
}
