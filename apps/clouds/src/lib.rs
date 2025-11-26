#![no_std]

extern crate alloc;
use userland::{canon, emit_window_buffer_updated, fiat, map, that, Value};
use uuid::Uuid;

pub struct AppHandle {
    pub window: Uuid,
    pub pixmap: Uuid,
}

pub fn register(compositor: Uuid) -> AppHandle {
    let window = Uuid::new_v5(&Uuid::NAMESPACE_OID, b"window-clouds0");
    let pixmap = Uuid::new_v5(&Uuid::NAMESPACE_OID, b"pixmap-clouds0");

    let mut fields = map();
    fields.insert(canon::NAME, Value::text("Clouds"));
    fields.insert(canon::TARGET, Value::uuid(pixmap));
    fields.insert(canon::STATUS, Value::symbol(canon::INIT));
    fiat(Some(window), canon::WINDOW, fields);
    that(window, canon::COMPOSED_BY, compositor, 0);

    AppHandle { window, pixmap }
}

pub fn tick(handle: &AppHandle, tick: u64) {
    static CLOUD_FRAMES: [&str; 2] = [
        "~~  ~ ~~~     ~~~\n ~~~   ~~  ~~ ~~  \n   ~~~ ~   ~~   ~~",
        " ~~~ ~   ~~   ~~ \n~~  ~~~  ~~~   ~~ \n   ~~~ ~~~ ~   ~~ ",
    ];
    if tick % 16 != 0 {
        return;
    }
    let frame = CLOUD_FRAMES[(tick as usize / 16) % CLOUD_FRAMES.len()];
    emit_window_buffer_updated(handle.window, handle.pixmap, tick, frame.as_bytes());
}
