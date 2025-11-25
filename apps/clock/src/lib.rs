#![no_std]

extern crate alloc;

use alloc::string::String;
use core::fmt::Write;
use userland::{
    canon, emit_edge_added, emit_thing_created, emit_window_buffer_updated, map, Value,
};
use uuid::Uuid;

pub struct AppHandle {
    pub window: Uuid,
    pub pixmap: Uuid,
}

pub fn register(compositor: Uuid) -> AppHandle {
    let window = Uuid::new_v5(&Uuid::NAMESPACE_OID, b"window-clock0");
    let pixmap = Uuid::new_v5(&Uuid::NAMESPACE_OID, b"pixmap-clock0");

    let mut fields = map();
    fields.insert(canon::NAME, Value::text("Clock"));
    fields.insert(canon::TARGET, Value::uuid(pixmap));
    fields.insert(canon::STATUS, Value::symbol(canon::INIT));
    emit_thing_created(window, canon::WINDOW, 0, fields);
    emit_edge_added(window, canon::COMPOSED_BY, compositor, 0);

    AppHandle { window, pixmap }
}

pub fn tick(handle: &AppHandle, tick: u64) {
    if tick % 4 != 0 {
        return;
    }
    let seconds = tick / 4;
    let minutes = seconds / 60;
    let hours = minutes / 60;
    let mut text = String::new();
    let _ = write!(
        &mut text,
        "Clock\n{:02}:{:02}:{:02}\nframe {}",
        hours % 24,
        minutes % 60,
        seconds % 60,
        tick
    );
    emit_window_buffer_updated(handle.window, handle.pixmap, tick, text.as_bytes());
}
