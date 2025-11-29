#![no_std]
#![no_main]

extern crate alloc;

use alloc::collections::BTreeMap;
use userland::prelude::*;
use userland::{canon, sys};
use uuid::Uuid;

pub struct MouseDriver {
    device_id: Uuid,
    device_handle: Option<u64>,
    irq_handle: Option<u64>,
    move_watch: WatchId,
    button_watch: WatchId,
}

impl App for MouseDriver {
    fn init(ctx: &mut AppContext<'_>) -> Self {
        let device_id = device_id();
        let device_handle = sys::dev_open(sys::DEVICE_KIND_MOUSE, 0);
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
            device_handle,
            irq_handle,
            move_watch,
            button_watch,
        }
    }

    fn tick(&mut self, _ctx: &mut AppContext<'_>, _tick: u64) {
        if let Some(handle) = self.irq_handle {
            let mut buf = [0u8; 16];
            if let Some(dev) = self.device_handle {
                loop {
                    let n = sys::dev_read(dev, &mut buf);
                    if n == 0 {
                        break;
                    }
                    for i in 0..n {
                        self.handle_byte(buf[i]);
                    }
                }
            }
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
                // println!("mouse move dx={} dy={}", dx, dy);
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
                // println!("mouse buttons 0b{:03b} down={}", buttons, down);
            }
            _ => {}
        }
    }
}

static mut MOUSE_DECODER: PacketDecoder = PacketDecoder::new();

impl MouseDriver {
    fn handle_byte(&self, byte: u8) {
        let event = unsafe { MOUSE_DECODER.feed(byte) };
        if let Some(event) = event {
            self.emit_mouse_event(event);
        }
    }

    fn emit_mouse_event(&self, event: MouseEvent) {
        let mut move_fields = BTreeMap::new();
        move_fields.insert(canon::DEVICE_ID, Value::Uuid(self.device_id));
        move_fields.insert(canon::DX, Value::I64(event.dx as i64));
        move_fields.insert(canon::DY, Value::I64(event.dy as i64));
        move_fields.insert(canon::BUTTONS, Value::U64(event.buttons as u64));
        // move_fields.insert(canon::TS, Value::U64(0));

        let _ = fiat(None, canon::MOUSE_MOVE, move_fields);

        if event.buttons_changed {
            let mut button_fields = BTreeMap::new();
            button_fields.insert(canon::DEVICE_ID, Value::Uuid(self.device_id));
            button_fields.insert(canon::BUTTONS, Value::U64(event.buttons as u64));
            button_fields.insert(canon::DOWN, Value::Bool(event.buttons != 0));
            // button_fields.insert(canon::TS, Value::U64(0));

            let _ = fiat(None, canon::MOUSE_BUTTON, button_fields);
        }
    }
}

#[derive(Clone, Copy, Debug, Default)]
struct MouseEvent {
    dx: i8,
    dy: i8,
    buttons: u8,
    buttons_changed: bool,
}

#[derive(Clone, Copy)]
struct PacketDecoder {
    packet: [u8; 3],
    index: usize,
    buttons: u8,
}

impl PacketDecoder {
    const fn new() -> Self {
        Self {
            packet: [0; 3],
            index: 0,
            buttons: 0,
        }
    }

    fn feed(&mut self, byte: u8) -> Option<MouseEvent> {
        if self.index == 0 && byte & 0x08 == 0 {
            return None;
        }

        self.packet[self.index] = byte;
        self.index = (self.index + 1) % 3;

        if self.index != 0 {
            return None;
        }

        let flags = self.packet[0];
        let dx = self.packet[1] as i8;
        let dy = (self.packet[2] as i8).wrapping_neg();

        if flags & 0x40 != 0 || flags & 0x80 != 0 {
            return None;
        }

        let buttons = flags & 0x07;
        let buttons_changed = buttons != self.buttons;
        self.buttons = buttons;

        Some(MouseEvent {
            dx,
            dy,
            buttons,
            buttons_changed,
        })
    }
}

const MOUSE_DEVICE_NAME: &str = "ps2-mouse0";
const MOUSE_IRQ_LINE: u8 = 12;

fn device_id() -> Uuid {
    Uuid::new_v5(&Uuid::NAMESPACE_OID, MOUSE_DEVICE_NAME.as_bytes())
}

app_main!(MouseDriver);
