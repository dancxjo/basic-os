#![no_std]

extern crate alloc;

use alloc::collections::BTreeMap;
use alloc::fmt::Write;
use alloc::string::{String, ToString};
use userland::{canon, extract_text, fetch_journal_events, Value};
use uuid::Uuid;

pub struct Compositor {
    last_seen: u64,
    frame_no: u64,
    windows: BTreeMap<Uuid, String>,
}

impl Compositor {
    pub const fn new() -> Self {
        Self {
            last_seen: 0,
            frame_no: 0,
            windows: BTreeMap::new(),
        }
    }

    /// Ingest new journal events and produce a composed frame string.
    pub fn tick(&mut self) -> String {
        self.ingest_events();
        let frame = self.compose_frame();
        self.frame_no = self.frame_no.wrapping_add(1);
        frame
    }

    fn ingest_events(&mut self) {
        let events = match fetch_journal_events() {
            Some(evts) => evts,
            None => return,
        };
        for evt in events {
            if evt.timestamp <= self.last_seen {
                continue;
            }
            self.last_seen = evt.timestamp;
            if evt.kind == canon::WINDOW_BUFFER_UPDATED {
                self.ingest_window_buffer(evt.data);
            }
        }
    }

    fn ingest_window_buffer(&mut self, data: Value) {
        let map = match data {
            Value::Map(m) => m,
            _ => return,
        };
        let window = map.get(&canon::SRC).and_then(Value::as_uuid);
        let text = map.get(&canon::TEXT).and_then(extract_text);
        if let (Some(win), Some(txt)) = (window, text) {
            self.windows.insert(win, txt);
        }
    }

    fn compose_frame(&self) -> String {
        let mut frame = String::new();
        let _ = writeln!(&mut frame, "[frame {}]", self.frame_no);
        for (idx, (win, text)) in self.windows.iter().enumerate() {
            let _ = writeln!(&mut frame, "-- win{} {} --", idx, short_id(*win));
            let _ = writeln!(&mut frame, "{}", text);
        }
        frame
    }
}

fn short_id(id: Uuid) -> alloc::string::String {
    let full = id.as_hyphenated().to_string();
    let mut short = String::new();
    for (i, ch) in full.chars().enumerate() {
        if i >= 8 {
            break;
        }
        short.push(ch);
    }
    short
}
