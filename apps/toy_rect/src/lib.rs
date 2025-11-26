#![no_std]

extern crate alloc;

use alloc::collections::BTreeMap;
use userland::prelude::*;
use userland::{canon, fiat, that, Value, graph};
use uuid::Uuid;

pub struct ToyRectApp {
    window_id: Uuid,
    rect_id: Uuid,
    color: u32,
    key_watch: Option<graph::WatchHandle>,
}

impl App for ToyRectApp {
    fn init(_ctx: &mut AppContext<'_>) -> Self {
        // 1. Create App Node
        let mut app_fields = BTreeMap::new();
        app_fields.insert(canon::KIND, Value::Symbol(canon::APP));
        app_fields.insert(canon::NAME, Value::Text("toy-rect".into()));
        let app_id = fiat(None, canon::APP, app_fields);

        // 2. Create Window Node
        let mut win_fields = BTreeMap::new();
        win_fields.insert(canon::KIND, Value::Symbol(canon::WINDOW));
        win_fields.insert(canon::TITLE, Value::Text("Toy Rect".into()));
        win_fields.insert(canon::X, Value::U64(50));
        win_fields.insert(canon::Y, Value::U64(50));
        win_fields.insert(canon::WIDTH, Value::U64(200));
        win_fields.insert(canon::HEIGHT, Value::U64(150));
        win_fields.insert(canon::VISIBLE, Value::Bool(true));
        let window_id = fiat(None, canon::WINDOW, win_fields);

        // Link App -> Window
        that(app_id, canon::OWNS, window_id, 0);

        // 3. Create Content Rect Node
        let color = 0xFF00FF00; // Green
        let mut rect_fields = BTreeMap::new();
        rect_fields.insert(canon::KIND, Value::Symbol(canon::WINDOW_RECT));
        rect_fields.insert(canon::X, Value::U64(0));
        rect_fields.insert(canon::Y, Value::U64(0));
        rect_fields.insert(canon::WIDTH, Value::U64(200));
        rect_fields.insert(canon::HEIGHT, Value::U64(150));
        rect_fields.insert(canon::COLOR, Value::U64(color as u64));
        let rect_id = fiat(None, canon::WINDOW_RECT, rect_fields);

        // Link Window -> Rect
        that(window_id, canon::HAS_CONTENT, rect_id, 0);

        ToyRectApp {
            window_id,
            rect_id,
            color,
            key_watch: None,
        }
    }

    fn tick(&mut self, _ctx: &mut AppContext<'_>, _tick: u64) {
        if self.key_watch.is_none() {
             let query = graph::WatchQuery {
                kind: Some(canon::KEY_PRESSED),
                src: None,
                dst: None,
            };
            self.key_watch = graph::watch(query);
        }

        if let Some(watch) = &self.key_watch {
            let events = graph::poll_watch(watch);
            for event in events {
                if let graph::GraphChange::Thing(_thing) = event {
                    // Cycle color on any key press
                    self.color = match self.color {
                        0xFF00FF00 => 0xFFFF0000, // Red
                        0xFFFF0000 => 0xFF0000FF, // Blue
                        _ => 0xFF00FF00,          // Green
                    };
                    
                    let mut updates = BTreeMap::new();
                    updates.insert(canon::COLOR, Value::U64(self.color as u64));
                    fiat(Some(self.rect_id), canon::WINDOW_RECT, updates);
                }
            }
        }
    }
}

app_main!(ToyRectApp);
