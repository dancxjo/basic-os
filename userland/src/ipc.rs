use alloc::vec;
use alloc::vec::Vec;

use crate::graph::{map, Value};
use crate::{canon, sys, Symbol};
use uuid::Uuid;

fn emit(kind: Symbol, data: Value) {
    if let Ok(buf) = postcard::to_allocvec(&data) {
        crate::println!("emit: calling syscall");
        let _ = sys::journal_emit_raw(kind.0, &buf);
        crate::println!("emit: syscall returned");
    }
    crate::println!("emit: returning");
}

#[deprecated(note = "use graph::fiat instead")]
pub fn emit_thing_created(id: Uuid, kind: Symbol, revision: u64, fields: crate::graph::Map) {
    crate::println!("emit_thing_created: start");
    let mut data = map();
    crate::println!("emit_thing_created: inserting ID");
    data.insert(canon::ID, Value::Uuid(id));
    crate::println!("emit_thing_created: inserting KIND");
    data.insert(canon::KIND, Value::Symbol(kind));
    crate::println!("emit_thing_created: inserting REVISION");
    data.insert(canon::REVISION, Value::U64(revision));
    crate::println!("emit_thing_created: inserting FIELDS");
    crate::println!("emit_thing_created: fields: {:?}", fields); 
    data.insert(canon::FIELDS, Value::Map(fields));
    crate::println!("emit_thing_created: calling emit");
    emit(canon::THING_CREATED, Value::Map(data));
    crate::println!("emit_thing_created: done");
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
