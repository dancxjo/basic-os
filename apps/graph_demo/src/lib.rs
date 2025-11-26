#![no_std]

extern crate alloc;

use alloc::string::{String, ToString};
use core::fmt::Write;
use userland::prelude::*;

pub struct GraphDemoApp {
    pub window: WindowHandle,
}

impl App for GraphDemoApp {
    fn init(ctx: &mut AppContext) -> Self {
        let window_data = Window {
            title: "Graph Demo".to_string(),
            x: 50,
            y: 50,
            width: 400,
            height: 300,
        };
        let window = ctx.create_window_with(window_data);
        GraphDemoApp { window }
    }

    fn tick(&mut self, ctx: &mut AppContext, tick: u64) {
        if tick % 16 != 0 {
            return;
        }

        let mut text = String::new();
        let _ = writeln!(&mut text, "Graph Snapshot Demo");
        let _ = writeln!(&mut text, "Tick: {}", tick);
        let _ = writeln!(&mut text, "");

        if let Some(snapshot) = graph_snapshot() {
            let _ = writeln!(&mut text, "Graph Revision: {}", snapshot.revision);
            let _ = writeln!(&mut text, "Things: {}", snapshot.thing_count);
            let _ = writeln!(&mut text, "Edges: {}", snapshot.edge_count);
            let _ = writeln!(&mut text, "");

            let windows = load_things_of_kind::<Window>();
            let _ = writeln!(&mut text, "Windows in graph: {}", windows.len());
            for (_id, window) in windows.iter() {
                let _ = writeln!(
                    &mut text,
                    "  '{}' at ({},{})",
                    window.title, window.x, window.y
                );
            }

            if let Some(my_window) = ctx.load_window(&self.window) {
                let _ = writeln!(&mut text, "");
                let _ = writeln!(&mut text, "My window:");
                let _ = writeln!(
                    &mut text,
                    "  Size: {}x{}",
                    my_window.width, my_window.height
                );

                if tick % 64 == 0 {
                    let mut updated = my_window.clone();
                    updated.width = 400 + (tick % 200);
                    updated.height = 300 + (tick % 150);
                    ctx.update_window(&self.window, &updated);
                    let _ = writeln!(&mut text, "  [Updated size!]");
                }
            }
        } else {
            let _ = writeln!(&mut text, "Failed to get graph snapshot");
        }

        ctx.clear_window(&self.window);
        ctx.draw_text(&self.window, format_args!("{}", text));
    }
}

app_main!(GraphDemoApp);
