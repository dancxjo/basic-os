#![no_std]

extern crate alloc;

use alloc::string::{String, ToString};
use core::fmt::Write;
use userland::{
    canon, emit_window_buffer_updated, fiat_thing, graph_snapshot, load_thing,
    load_things_of_kind, that, update_thing, Window,
};
use uuid::Uuid;

pub struct AppHandle {
    pub window: Uuid,
    pub pixmap: Uuid,
}

pub fn register(compositor: Uuid) -> AppHandle {
    let window_data = Window {
        title: "Graph Demo".to_string(),
        x: 50,
        y: 50,
        width: 400,
        height: 300,
    };

    let window = fiat_thing(&window_data);
    let pixmap = Uuid::new_v5(&Uuid::NAMESPACE_OID, b"pixmap-graph-demo");

    that(window, canon::COMPOSED_BY, compositor, 0);

    AppHandle { window, pixmap }
}

pub fn tick(handle: &AppHandle, tick: u64) {
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
            let _ = writeln!(&mut text, "  '{}' at ({},{})", window.title, window.x, window.y);
        }

        if let Some(my_window) = load_thing::<Window>(handle.window) {
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
                update_thing(handle.window, &updated);
                let _ = writeln!(&mut text, "  [Updated size!]");
            }
        }
    } else {
        let _ = writeln!(&mut text, "Failed to get graph snapshot");
    }

    emit_window_buffer_updated(handle.window, handle.pixmap, tick, text.as_bytes());
}
