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
    selection_node_id: Uuid,
    is_active: bool,
    window_watch: Option<userland::watch::WatchId>,
    selection_watch: Option<userland::watch::WatchId>,
}

struct LauncherEntry {
    fs_node_id: Uuid,
    label: alloc::string::String,
}

impl App for LauncherApp {
    fn init(ctx: &mut AppContext<'_>) -> Self {
        let window = ctx.create_window("Launcher");

        // Create launcher surface widget (container)
        let launcher_surface_id = userland::simple_uuid(b"launcher_surface");
        let mut fields = graph::map();
        fields.insert(canon::ROLE, Value::Text("launcher_surface".to_string()));
        fields.insert(canon::LABEL, Value::Text("Launcher".to_string()));
        fields.insert(canon::VISIBLE, Value::Bool(true));
        fields.insert(canon::PARENT, Value::Uuid(window.window_id()));
        graph::fiat(Some(launcher_surface_id), canon::WIDGET, fields);
        graph::that(window.window_id(), "contains", launcher_surface_id, 0);

        // Grant access to widget_host
        let widget_host_bundle = Uuid::new_v5(&Uuid::NAMESPACE_OID, b"widget_host");
        graph::grant_capability(widget_host_bundle, launcher_surface_id, "CAN_READ");

        let mut app = LauncherApp {
            window: window.clone(),
            entries: Vec::new(),
            selection_node_id: Uuid::nil(),
            is_active: false,
            window_watch: None,
            selection_watch: None,
        };

        app.sync_entries(ctx, launcher_surface_id);

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

            if Some(*watch) == self.selection_watch {
                // Selection changed
                if let Some(Value::I64(idx)) = thing.fields.get(&canon::SELECTED_INDEX) {
                    if *idx >= 0 {
                        self.activate_entry(*idx as usize);
                    }
                }
            }
        }
    }

    fn tick(&mut self, _ctx: &mut AppContext<'_>, _tick: u64) {
        // No manual drawing needed, widget handles it.
    }
}

impl LauncherApp {
    fn sync_entries(&mut self, ctx: &mut AppContext<'_>, parent_id: Uuid) {
        let widget_host_bundle = Uuid::new_v5(&Uuid::NAMESPACE_OID, b"widget_host");

        // Create selection node
        let selection_id = userland::simple_uuid(b"launcher_selection");
        let mut sel_fields = graph::map();
        sel_fields.insert(canon::KIND, Value::Symbol(canon::cc('V', 'A')));
        sel_fields.insert(canon::SELECTED_INDEX, Value::I64(-1));
        graph::fiat(Some(selection_id), canon::cc('V', 'A'), sel_fields);
        self.selection_node_id = selection_id;

        graph::grant_capability(widget_host_bundle, selection_id, "CAN_READ");
        graph::grant_capability(widget_host_bundle, selection_id, "CAN_WRITE");

        self.selection_watch = Some(ctx.watch_graph(ThingFilter {
            kind: None,
            id: Some(selection_id),
        }));

        // Create listbox widget (Control)
        let listbox_id = userland::simple_uuid(b"launcher_listbox");
        let mut list_fields = graph::map();
        list_fields.insert(canon::ROLE, Value::Text("primary_list".to_string()));
        list_fields.insert(
            canon::cc('W', 'K'),
            Value::Text("listbox_default".to_string()),
        );
        list_fields.insert(canon::VISIBLE, Value::Bool(true));
        list_fields.insert(canon::PARENT, Value::Uuid(parent_id));
        list_fields.insert(canon::WIDTH, Value::U64(300));
        list_fields.insert(canon::HEIGHT, Value::U64(400));
        list_fields.insert(canon::CONTROL_KIND, Value::Text("list".to_string()));

        // Bind to selection via property
        list_fields.insert(canon::BINDS, Value::Uuid(selection_id));

        // Read /bin
        if let Ok(bin_entries) = userland::fs::read_dir("/bin") {
            for entry in bin_entries {
                if !entry.show_in_launcher {
                    continue;
                }

                let item_id = userland::simple_uuid(
                    alloc::format!("launcher_item_{}", entry.name).as_bytes(),
                );
                let mut item_fields = graph::map();
                item_fields.insert(canon::ITEM_LABEL, Value::Text(entry.name.clone()));
                item_fields.insert(canon::ITEM_VALUE, Value::Text(entry.name.clone()));
                // Set PARENT to listbox so widget can find it
                item_fields.insert(canon::PARENT, Value::Uuid(listbox_id));

                graph::fiat(Some(item_id), canon::ITEM, item_fields);
                graph::grant_capability(widget_host_bundle, item_id, "CAN_READ");

                self.entries.push(LauncherEntry {
                    fs_node_id: entry.id,
                    label: entry.name,
                });
            }
        }

        graph::fiat(Some(listbox_id), canon::WIDGET, list_fields);
        graph::grant_capability(widget_host_bundle, listbox_id, "CAN_READ");
        graph::that(parent_id, "contains", listbox_id, 0);
    }

    fn activate_entry(&self, idx: usize) {
        if let Some(entry) = self.entries.get(idx) {
            if let Ok(node) = userland::fs::get_node_by_id(entry.fs_node_id) {
                if let Some(bin_name) = node.bin_name {
                    userland::println!("Launcher launching: {}", bin_name);
                    userland::sys::spawn(&bin_name);
                }
            }
        }
    }
}
