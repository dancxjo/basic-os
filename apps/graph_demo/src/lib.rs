#![no_std]

extern crate alloc;

use alloc::string::{String, ToString};
use core::fmt::Write;
use userland::prelude::*;
use uuid::Uuid;

pub struct GraphDemoApp {
    pub window: WindowHandle,
}

impl App for GraphDemoApp {
    fn init(ctx: &mut AppContext<'_>) -> Self {
        let window_data = Window {
            id: Uuid::nil(),
            title: "Graph Demo".to_string(),
            x: 50,
            y: 50,
            width: 400,
            height: 300,
        };
        let window = ctx.create_window_with(window_data);
        GraphDemoApp { window }
    }

    fn tick(&mut self, ctx: &mut AppContext<'_>, tick: u64) {
        if tick % 16 != 0 {
            return;
        }

        let mut text = String::new();
        let _ = writeln!(&mut text, "Graph Snapshot Demo");
        let _ = writeln!(&mut text, "Tick: {}", tick);
        let _ = writeln!(&mut text, "");

        // Snapshot API removed. Using granular queries.
        {
            let windows = load_things_of_kind::<Window>();
            let _ = writeln!(&mut text, "Windows in graph: {}", windows.len());
            for (_id, window) in windows.iter() {
                let _ = writeln!(
                    &mut text,
                    "  '{}' at ({},{})",
                    window.title, window.x, window.y
                );
            }
        }

        ctx.clear_window(&self.window);
        ctx.draw_text(&self.window, format_args!("{}", text));
    }
}

app_main!(GraphDemoApp);
