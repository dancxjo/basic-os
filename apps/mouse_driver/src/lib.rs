#![no_std]
#![no_main]

extern crate alloc;

use userland::prelude::*;
use userland::{canon, sys};
use uuid::Uuid;

pub struct MouseDriver {
    device_id: Uuid,
    irq_handle: Option<u64>,
    move_watch: WatchId,
    button_watch: WatchId,
}

impl App for MouseDriver {
    fn init(ctx: &mut AppContext<'_>) -> Self {
        let device_id = device_id();
        let irq_handle = sys::irq_bind(sys::IrqBindRequest {
            device: device_id,
            irq_line: MOUSE_IRQ_LINE,
        });

        let move_watch = ctx.watch_graph(ThingFilter {
            kind: Some(canon::MOUSE_MOVE),
            id: None,
        });
        let button_watch = ctx.watch_graph(ThingFilter {
            kind: Some(canon::MOUSE_BUTTON),
            id: None,
        });

        Self {
            device_id,
            irq_handle,
            move_watch,
            button_watch,
        }
    }

    fn tick(&mut self, _ctx: &mut AppContext<'_>, _tick: u64) {
        if let Some(handle) = self.irq_handle {
            let _ = sys::irq_ack(handle);
        }
    }

    fn on_event(&mut self, _ctx: &mut AppContext<'_>, ev: AppEvent) {
        match ev {
            AppEvent::Thing { watch, thing } if watch == self.move_watch => {
                let dx = thing
                    .fields
                    .get(&canon::DX)
                    .and_then(|v| v.as_i64())
                    .unwrap_or(0);
                let dy = thing
                    .fields
                    .get(&canon::DY)
                    .and_then(|v| v.as_i64())
                    .unwrap_or(0);
                println!("mouse move dx={} dy={}", dx, dy);
            }
            AppEvent::Thing { watch, thing } if watch == self.button_watch => {
                let buttons = thing
                    .fields
                    .get(&canon::BUTTONS)
                    .and_then(|v| v.as_u64())
                    .unwrap_or(0);
                let down = thing
                    .fields
                    .get(&canon::DOWN)
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false);
                println!("mouse buttons 0b{:03b} down={}", buttons, down);
            }
            _ => {}
        }
    }
}

impl MouseDriver {}

const MOUSE_DEVICE_NAME: &str = "ps2-mouse0";
const MOUSE_IRQ_LINE: u8 = 12;

fn device_id() -> Uuid {
    Uuid::new_v5(&Uuid::NAMESPACE_OID, MOUSE_DEVICE_NAME.as_bytes())
}

app_main!(MouseDriver);
