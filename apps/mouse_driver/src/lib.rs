#![no_std]

extern crate alloc;

use userland::prelude::*;
use userland::{canon, graph, sys, Symbol};
use uuid::Uuid;

pub struct MouseDriver {
    decoder: PacketDecoder,
    device_id: Uuid,
    event_counter: u64,
}

impl App for MouseDriver {
    fn init(ctx: &mut AppContext<'_>) -> Self {
        let device_id = Uuid::new_v5(&Uuid::NAMESPACE_OID, b"mouse0");
        
        // Register the mouse device
        let mut fields = graph::map();
        fields.insert(canon::DEVICE_ID, graph::Value::text("mouse0"));
        fields.insert(canon::KIND, graph::Value::symbol(canon::INPUT_DEVICE_MOUSE));
        
        graph::fiat(Some(device_id), canon::INPUT_DEVICE_MOUSE, fields);

        Self {
            decoder: PacketDecoder::new(),
            device_id,
            event_counter: 0,
        }
    }

    fn tick(&mut self, _ctx: &mut AppContext<'_>, _tick: u64) {
        let mut buf = [0u8; 64];
        let count = unsafe { sys::syscall(sys::SYSCALL_MOUSE_READ, buf.as_mut_ptr() as u64, buf.len() as u64, 0) };
        
        for i in 0..count as usize {
            if let Some(event) = self.decoder.feed(buf[i]) {
                self.publish_event(event);
            }
        }
    }
}

impl MouseDriver {
    fn publish_event(&mut self, event: MouseEvent) {
        let mut fields = graph::map();
        fields.insert(canon::DEVICE_ID, graph::Value::text("mouse0"));
        fields.insert(canon::TS, graph::Value::U64(0)); // TODO: get time
        fields.insert(canon::KIND, graph::Value::symbol(canon::MOVE));
        fields.insert(canon::DX, graph::Value::I64(event.dx as i64));
        fields.insert(canon::DY, graph::Value::I64(event.dy as i64));
        
        let buttons: u64 = (event.left as u64) | ((event.right as u64) << 1) | ((event.middle as u64) << 2);
        fields.insert(canon::BUTTON, graph::Value::U64(buttons));
        fields.insert(canon::DOWN, graph::Value::Bool(event.left || event.right || event.middle));

        self.event_counter += 1;
        let event_id = Uuid::new_v5(&self.device_id, &self.event_counter.to_le_bytes());
        graph::fiat(Some(event_id), canon::INPUT_EVENT, fields);

        graph::that(self.device_id, canon::EMITS, event_id, 0);
    }
}

#[derive(Clone, Copy, Debug, Default)]
struct MouseEvent {
    dx: i8,
    dy: i8,
    left: bool,
    right: bool,
    middle: bool,
}

#[derive(Clone, Copy)]
struct PacketDecoder {
    packet: [u8; 3],
    index: usize,
}

impl PacketDecoder {
    const fn new() -> Self {
        Self {
            packet: [0; 3],
            index: 0,
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

        Some(MouseEvent {
            dx,
            dy,
            left: flags & 0x01 != 0,
            right: flags & 0x02 != 0,
            middle: flags & 0x04 != 0,
        })
    }
}

app_main!(MouseDriver);
