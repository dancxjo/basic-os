#![no_std]

extern crate alloc;

use alloc::collections::BTreeMap;
use alloc::fmt::Write;
use alloc::string::{String, ToString};
use userland::{canon, extract_text, AppEvent, EventFilter, Value, WatchId, WatchManager};
use uuid::Uuid;

pub struct Compositor {
    frame_no: u64,
    windows: BTreeMap<Uuid, String>,
    watch_window_buffers: Option<WatchId>,
}

impl Compositor {
    pub const fn new() -> Self {
        Self {
            frame_no: 0,
            windows: BTreeMap::new(),
            watch_window_buffers: None,
        }
    }

    pub fn init_with_watches(watch_manager: &mut WatchManager, app_id: usize) -> Self {
        let filter = EventFilter {
            kind: Some(canon::WINDOW_BUFFER_UPDATED),
            src: None,
            dst: None,
        };
        let watch_id = watch_manager.register_journal(app_id, filter);

        Self {
            frame_no: 0,
            windows: BTreeMap::new(),
            watch_window_buffers: Some(watch_id),
        }
    }

    pub fn on_event(&mut self, ev: &AppEvent) {
        if let AppEvent::Journal { watch, event } = ev {
            if Some(*watch) != self.watch_window_buffers {
                return;
            }
            if event.kind != canon::WINDOW_BUFFER_UPDATED {
                return;
            }
            self.ingest_window_buffer(event.data.clone());
        }
    }

    /// Produce a composed frame string and increment the frame counter.
    pub fn tick(&mut self) -> String {
        let frame = self.compose_frame();
        self.frame_no = self.frame_no.wrapping_add(1);
        frame
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
