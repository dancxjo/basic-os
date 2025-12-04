#![cfg_attr(not(feature = "std"), no_std)]

extern crate alloc;

use alloc::string::ToString;
use alloc::vec::Vec;
use userland::prelude::*;
use userland::{canon, graph, AppEvent, ThingFilter};
use uuid::Uuid;

const CLOUDS_BMP: &[u8] = include_bytes!("../../../clouds.bmp");

pub struct GraphViewerApp {
    window: WindowHandle,
    entries: Vec<GraphViewerEntry>,
    selection_node_id: Uuid,
    is_active: bool,
    window_watch: Option<userland::watch::WatchId>,
    selection_watch: Option<userland::watch::WatchId>,
}

struct GraphViewerEntry {
    fs_node_id: Uuid,
    label: alloc::string::String,
}

impl App for GraphViewerApp {
    fn init(ctx: &mut AppContext<'_>) -> Self {
        let mut window_fields = userland::graph::Window {
            id: Uuid::nil(),
            title: "Graph Viewer".to_string(),
            x: 0,
            y: 0,
            width: 320,
            height: 200,
            z: 0,
            visible: true,
            target: None,
            active: false,
            is_root: true,
            mode_index: Some(0), // F1
            window_rect: None,
        };
        let window = ctx.create_window_with(window_fields);

        let mut app = GraphViewerApp {
            window: window.clone(),
            entries: Vec::new(),
            selection_node_id: Uuid::nil(),
            is_active: false,
            window_watch: None,
            selection_watch: None,
        };

        app.sync_entries(ctx, window.window_id());

        // Draw background
        ctx.draw_bitmap(&window, CLOUDS_BMP);
        ctx.draw_text(&window, format_args!(""));

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

impl GraphViewerApp {
    fn sync_entries(&mut self, ctx: &mut AppContext<'_>, parent_id: Uuid) {
        let widget_host_bundle = Uuid::new_v5(&Uuid::NAMESPACE_OID, b"widget_host");

        // Create selection node
        let selection_id = userland::simple_uuid(b"graph_viewer_selection");
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
        let listbox_id = userland::simple_uuid(b"graph_viewer_listbox");
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
                if !entry.show_in_graph_viewer {
                    continue;
                }

                let item_id = userland::simple_uuid(
                    alloc::format!("graph_viewer_item_{}", entry.name).as_bytes(),
                );
                let mut item_fields = graph::map();
                item_fields.insert(canon::ROLE, Value::Text("list_item".to_string()));
                item_fields.insert(canon::ITEM_LABEL, Value::Text(entry.name.clone()));
                item_fields.insert(canon::ITEM_VALUE, Value::Text(entry.name.clone()));
                // Set PARENT to listbox so widget can find it
                item_fields.insert(canon::PARENT, Value::Uuid(listbox_id));
                item_fields.insert(canon::HEIGHT, Value::U64(20));

                graph::fiat(Some(item_id), canon::WIDGET, item_fields);
                graph::grant_capability(widget_host_bundle, item_id, "CAN_READ");

                self.entries.push(GraphViewerEntry {
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
                    userland::println!("Graph Viewer launching: {}", bin_name);
                    userland::sys::spawn(&bin_name);
                }
            }
        }
    }
}
