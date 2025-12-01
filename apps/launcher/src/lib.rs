#![cfg_attr(not(feature = "std"), no_std)]

extern crate alloc;

use alloc::string::ToString;
use alloc::vec::Vec;
use userland::prelude::*;
use userland::{canon, graph, AppEvent, ThingFilter};
use uuid::Uuid;

pub struct LauncherApp {
    window: WindowHandle,
    entries: Vec<LauncherEntry>,
    focused_index: Option<usize>,
    launcher_surface_id: Uuid,
    is_active: bool,
    window_watch: Option<userland::watch::WatchId>,
}

struct LauncherEntry {
    widget_id: Uuid,
    fs_node_id: Uuid,
    label: alloc::string::String,
}

impl App for LauncherApp {
    fn init(ctx: &mut AppContext<'_>) -> Self {
        let window = ctx.create_window("Launcher");

        // Create launcher surface widget
        let launcher_surface_id = userland::simple_uuid(b"launcher_surface");
        let mut fields = graph::map();
        fields.insert(canon::ROLE, Value::Text("launcher_surface".to_string()));
        fields.insert(canon::LABEL, Value::Text("Launcher".to_string()));
        fields.insert(canon::VISIBLE, Value::Bool(true));
        // Attach to window
        fields.insert(canon::PARENT, Value::Uuid(window.window_id()));

        graph::fiat(Some(launcher_surface_id), canon::WIDGET, fields);

        // Link window to surface
        graph::that(window.window_id(), "contains", launcher_surface_id, 0);

        // Sync entries
        let mut app = LauncherApp {
            window: window.clone(),
            entries: Vec::new(),
            focused_index: None,
            launcher_surface_id,
            is_active: false,
            window_watch: None,
        };

        app.sync_entries(ctx);

        // Watch for key events on this window
        ctx.watch_graph(ThingFilter {
            kind: Some(canon::KEY_EVENT),
            id: None,
        });

        // Watch window for active state
        app.window_watch = Some(ctx.watch_graph(ThingFilter {
            kind: None,
            id: Some(window.window_id()),
        }));

        app
    }

    fn on_event(&mut self, ctx: &mut AppContext<'_>, ev: AppEvent) {
        if let AppEvent::Thing { watch, thing } = &ev {
            if Some(*watch) == self.window_watch {
                self.is_active = thing
                    .fields
                    .get(&canon::ACTIVE)
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false);
            }

            if thing.kind == canon::KEY_EVENT {
                if !self.is_active {
                    return;
                }

                let down = thing
                    .fields
                    .get(&canon::DOWN)
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false);

                if !down {
                    return;
                }

                let key = thing.fields.get(&canon::KEY).and_then(|v| v.as_symbol());

                if let Some(k) = key {
                    if k == canon::cc('U', 'P') {
                        self.focus_prev(ctx);
                    } else if k == canon::cc('D', 'N') {
                        self.focus_next(ctx);
                    } else if k == canon::cc('E', 'N') || k == canon::from_char(' ') {
                        self.activate_focused(ctx);
                    }
                }
            }
        }
    }

    fn tick(&mut self, ctx: &mut AppContext<'_>, _tick: u64) {
        self.draw(ctx);
    }
}

impl LauncherApp {
    fn sync_entries(&mut self, ctx: &mut AppContext<'_>) {
        // Read /bin
        if let Ok(bin_entries) = userland::fs::read_dir("/bin") {
            for entry in bin_entries {
                if !entry.show_in_launcher {
                    continue;
                }

                let entry_id = userland::simple_uuid(
                    alloc::format!("launcher_entry_{}", entry.name).as_bytes(),
                );
                let mut entry_fields = graph::map();
                entry_fields.insert(canon::ROLE, Value::Text("launcher_entry".to_string()));
                entry_fields.insert(canon::LABEL, Value::Text(entry.name.clone()));
                entry_fields.insert(canon::VISIBLE, Value::Bool(true));
                entry_fields.insert(canon::FOCUSABLE, Value::Bool(true));
                entry_fields.insert(canon::PARENT, Value::Uuid(self.launcher_surface_id));

                graph::fiat(Some(entry_id), canon::WIDGET, entry_fields);

                // Link it to the launcher surface with HAS_ENTRY.
                graph::that(self.launcher_surface_id, "HAS_ENTRY", entry_id, 0);
                // Also link with "contains" for generic widget hierarchy?
                graph::that(self.launcher_surface_id, "contains", entry_id, 0);

                // Link to the FsNode with LAUNCHES.
                graph::that(entry_id, "LAUNCHES", entry.id, 0);

                self.entries.push(LauncherEntry {
                    widget_id: entry_id,
                    fs_node_id: entry.id,
                    label: entry.name,
                });
            }
        }

        if !self.entries.is_empty() {
            self.focused_index = Some(0);
            self.update_focus_visuals(ctx);
        }
    }

    fn focus_next(&mut self, ctx: &mut AppContext<'_>) {
        if let Some(idx) = self.focused_index {
            if idx + 1 < self.entries.len() {
                self.focused_index = Some(idx + 1);
                self.update_focus_visuals(ctx);
            }
        } else if !self.entries.is_empty() {
            self.focused_index = Some(0);
            self.update_focus_visuals(ctx);
        }
    }

    fn focus_prev(&mut self, ctx: &mut AppContext<'_>) {
        if let Some(idx) = self.focused_index {
            if idx > 0 {
                self.focused_index = Some(idx - 1);
                self.update_focus_visuals(ctx);
            }
        } else if !self.entries.is_empty() {
            self.focused_index = Some(0);
            self.update_focus_visuals(ctx);
        }
    }

    fn activate_focused(&self, _ctx: &mut AppContext<'_>) {
        if let Some(idx) = self.focused_index {
            if let Some(entry) = self.entries.get(idx) {
                // Launch it!
                if let Ok(node) = userland::fs::get_node_by_id(entry.fs_node_id) {
                    if let Some(bin_name) = node.bin_name {
                        userland::println!("Launcher launching: {}", bin_name);
                        userland::sys::spawn(&bin_name);
                    }
                }
            }
        }
    }

    fn update_focus_visuals(&self, ctx: &mut AppContext<'_>) {
        for (i, entry) in self.entries.iter().enumerate() {
            let is_focused = Some(i) == self.focused_index;
            // Update the widget's FOCUSED property
            let mut updates = graph::map();
            updates.insert(canon::FOCUSED, Value::Bool(is_focused));
            graph::fiat(Some(entry.widget_id), canon::WIDGET, updates);
        }
        self.draw(ctx);
    }

    fn draw(&self, ctx: &mut AppContext<'_>) {
        ctx.clear_window(&self.window);

        for (i, entry) in self.entries.iter().enumerate() {
            let is_focused = Some(i) == self.focused_index;
            let prefix = if is_focused { "> " } else { "  " };
            ctx.draw_text(
                &self.window,
                core::format_args!("{}{}\n", prefix, entry.label),
            );
        }
    }
}
