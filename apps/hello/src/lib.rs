#![no_std]

extern crate alloc;

use alloc::string::String;
use core::fmt::Write;
use userland::{canon, emit_window_buffer_updated, fiat, map, that, Value};
use uuid::Uuid;

pub struct AppHandle {
    pub window: Uuid,
    pub pixmap: Uuid,
}

pub fn register(compositor: Uuid) -> AppHandle {
    let window = Uuid::new_v5(&Uuid::NAMESPACE_OID, b"window-hello0");
    let pixmap = Uuid::new_v5(&Uuid::NAMESPACE_OID, b"pixmap-hello0");

    let mut fields = map();
    fields.insert(canon::NAME, Value::text("Hello"));
    fields.insert(canon::TARGET, Value::uuid(pixmap));
    fields.insert(canon::STATUS, Value::symbol(canon::INIT));
    fiat(Some(window), canon::WINDOW, fields);
    that(window, canon::COMPOSED_BY, compositor, 0);

    AppHandle { window, pixmap }
}

pub fn tick(handle: &AppHandle, tick: u64) {
    if tick % 8 != 0 {
        return;
    }
    let mut text = String::new();
    let _ = write!(
        &mut text,
        "Hello from app {}\nrev {}\nEnjoy the clouds.",
        "hello", tick
    );
    emit_window_buffer_updated(handle.window, handle.pixmap, tick, text.as_bytes());
}
