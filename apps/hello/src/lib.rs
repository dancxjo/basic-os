#![no_std]

extern crate alloc;

use userland::prelude::*;
use userland::{canon, graph, Symbol};
use uuid::Uuid;

pub struct HelloApp {
    window: WindowHandle,
    key_watch: Option<graph::WatchHandle>,
}

impl App for HelloApp {
    fn init(ctx: &mut AppContext<'_>) -> Self {
        let window = ctx.create_window("Hello");
        
        // Watch for key presses
        let query = graph::WatchQuery {
            kind: Some(canon::KEY_PRESSED),
            src: None,
            dst: None,
        };
        let key_watch = graph::watch(query);

        HelloApp { window, key_watch }
    }

    fn tick(&mut self, ctx: &mut AppContext<'_>, tick: u64) {
        // Poll key events
        if let Some(watch) = &self.key_watch {
            let events = graph::poll_watch(watch);
            for event in events {
                if let graph::GraphChange::Thing(thing) = event {
                    if let Some(text) = thing.fields.get(&canon::TEXT).and_then(|v| v.as_text()) {
                        userland::println!("Key pressed: {}", text);
                    }
                }
            }
        }

        if tick % 8 != 0 {
            return;
        }
        ctx.clear_window(&self.window);
        ctx.draw_text(
            &self.window,
            format_args!(
                "Hello from app {}\nrev {}\nPress keys to see logs.",
                "hello", tick
            ),
        );
    }
}

app_main!(HelloApp);
