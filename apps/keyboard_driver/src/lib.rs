#![no_std]
#![no_main]

extern crate alloc;

use userland::prelude::*;
use userland::{canon, sys};
use uuid::Uuid;

pub struct KeyboardDriver {
    device_id: Uuid,
    irq_handle: Option<u64>,
    watch: WatchId,
}

impl App for KeyboardDriver {
    fn init(ctx: &mut AppContext<'_>) -> Self {
        let device_id = device_id();
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
            irq_handle,
            watch,
        }
    }

    fn tick(&mut self, _ctx: &mut AppContext<'_>, _tick: u64) {
        if let Some(handle) = self.irq_handle {
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
                println!("key event: scancode=0x{:02x} down={}", scancode, down);
            }
        }
    }
}

impl KeyboardDriver {}

const KEYBOARD_DEVICE_NAME: &str = "ps2-keyboard0";
const KEYBOARD_IRQ_LINE: u8 = 1;

fn device_id() -> Uuid {
    Uuid::new_v5(&Uuid::NAMESPACE_OID, KEYBOARD_DEVICE_NAME.as_bytes())
}

app_main!(KeyboardDriver);
