#![no_std]

extern crate alloc;

use alloc::collections::BTreeMap;
use userland::prelude::*;
use userland::{canon, fiat, sys, that, Value};
use uuid::Uuid;

pub struct KeyboardDriver {
    device_id: Uuid,
}

impl App for KeyboardDriver {
    fn init(_ctx: &mut AppContext<'_>) -> Self {
        let mut fields = BTreeMap::new();
        fields.insert(canon::KIND, Value::Symbol(canon::KEYBOARD));
        fields.insert(canon::NAME, Value::Text("ps2-keyboard".into()));
        
        // Fiat the keyboard device
        let device_id = fiat(None, canon::KEYBOARD, fields);
        
        KeyboardDriver { device_id }
    }

    fn tick(&mut self, _ctx: &mut AppContext<'_>, _tick: u64) {
        let mut buf = [0u8; 32];
        let count = sys::kbd_read_raw(&mut buf) as usize;
        
        for i in 0..count {
            let scancode = buf[i];
            self.publish_key_event(scancode);
        }
    }
}

impl KeyboardDriver {
    fn publish_key_event(&self, scancode: u8) {
        let mut fields = BTreeMap::new();
        fields.insert(canon::SCANCODE, Value::U64(scancode as u64));
        // Simple mapping for demo purposes (A=0x1E)
        let key_char = match scancode {
            0x1E => "A",
            0x30 => "B",
            0x2E => "C",
            _ => "?",
        };
        fields.insert(canon::TEXT, Value::Text(key_char.into()));

        let event_id = fiat(None, canon::KEY_PRESSED, fields);
        that(self.device_id, canon::EMITS, event_id, 0);
    }
}

app_main!(KeyboardDriver);
