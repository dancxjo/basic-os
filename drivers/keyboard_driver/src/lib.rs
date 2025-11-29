#![no_std]
#![no_main]

extern crate alloc;

use userland::prelude::*;
use userland::{canon, sys};
use uuid::Uuid;

pub struct KeyboardDriver {
    device_id: Uuid,
    device_handle: Option<u64>,
    irq_handle: Option<u64>,
    watch: WatchId,
}

impl App for KeyboardDriver {
    fn init(ctx: &mut AppContext<'_>) -> Self {
        let device_id = device_id();
        let device_handle = sys::dev_open(sys::DEVICE_KIND_KEYBOARD, 0);
        let irq_handle = sys::irq_bind(sys::IrqBindRequest {
            device: device_id,
            irq_line: KEYBOARD_IRQ_LINE,
        });

        let watch = ctx.watch_graph(ThingFilter {
            kind: Some(canon::KEY_EVENT),
            id: None,
        });

        KeyboardDriver {
            device_id,
            device_handle,
            irq_handle,
            watch,
        }
    }

    fn tick(&mut self, _ctx: &mut AppContext<'_>, _tick: u64) {
        if let Some(handle) = self.irq_handle {
            // Read all available scancodes
            let mut buf = [0u8; 16];
            if let Some(dev) = self.device_handle {
                loop {
                    let n = sys::dev_read(dev, &mut buf);
                    if n == 0 {
                        break;
                    }
                    for i in 0..n {
                        self.handle_scancode(buf[i]);
                    }
                }
            }
            let _ = sys::irq_ack(handle);
        }
    }

    fn on_event(&mut self, _ctx: &mut AppContext<'_>, ev: AppEvent) {
        if let AppEvent::Thing { watch, thing } = ev {
            if watch == self.watch && thing.kind == canon::KEY_EVENT {
                let scancode = thing
                    .fields
                    .get(&canon::SCANCODE)
                    .and_then(|v| v.as_u64())
                    .unwrap_or(0);
                let down = thing
                    .fields
                    .get(&canon::DOWN)
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false);
                // println!("key event: scancode=0x{:02x} down={}", scancode, down);
            }
        }
    }
}

impl KeyboardDriver {
    fn handle_scancode(&self, scancode: u8) {
        let (code, down) = decode_scancode(scancode);

        let mut fields = alloc::collections::BTreeMap::new();
        fields.insert(canon::DEVICE_ID, Value::Uuid(self.device_id));
        fields.insert(canon::SCANCODE, Value::U64(code as u64));
        fields.insert(canon::DOWN, Value::Bool(down));
        // fields.insert(canon::TS, Value::U64(0)); // TODO: timestamp

        let _ = fiat(None, canon::KEY_EVENT, fields);
    }
}

fn decode_scancode(scancode: u8) -> (u8, bool) {
    if scancode & 0x80 != 0 {
        (scancode & 0x7F, false)
    } else {
        (scancode, true)
    }
}

const KEYBOARD_DEVICE_NAME: &str = "ps2-keyboard0";
const KEYBOARD_IRQ_LINE: u8 = 1;

fn device_id() -> Uuid {
    Uuid::new_v5(&Uuid::NAMESPACE_OID, KEYBOARD_DEVICE_NAME.as_bytes())
}

app_main!(KeyboardDriver);
