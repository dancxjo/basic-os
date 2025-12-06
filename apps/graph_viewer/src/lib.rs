#![cfg_attr(not(feature = "std"), no_std)]

extern crate alloc;

use alloc::collections::{BTreeMap, BTreeSet};
use alloc::format;
use alloc::string::ToString;
use thing_abi::GraphThing;
use userland::prelude::*;
use userland::{canon, graph, AppEvent, NodePattern, ThingFilter, Value};
use uuid::Uuid;

pub struct GraphViewerApp {
    window: WindowHandle,
    place_id: Option<Uuid>,
    root_widget: Uuid,
    widgets: BTreeMap<Uuid, Uuid>, // Thing ID -> Widget ID
    contents: BTreeSet<Uuid>,
    counter: u64,
}

const WALLPAPER: &[u8] = include_bytes!("../../../clouds.bmp");

impl App for GraphViewerApp {
    fn init(ctx: &mut AppContext<'_>) -> Self {
        let width = 1024;
        let height = 768;

        // Check for launch intent
        let (mut place_id, mode_index) = match GraphViewerApp::discover_launch_intent() {
            Some((place, mode, intent_id)) => {
                // Consume intent
                let mut updates = graph::map();
                updates.insert(canon::VISIBLE, Value::Bool(false));
                graph::fiat(Some(intent_id), canon::LAUNCH_INTENT, updates);
                (Some(place), mode)
            }
            None => (None, None),
        };

        let title = if let Some(pid) = place_id {
            let place_label = userland::graph::get_thing(pid)
                .and_then(|place| {
                    place
                        .fields
                        .get(&canon::NAME)
                        .and_then(|v| v.as_text().map(|s| s.to_string()))
                })
                .unwrap_or_else(|| "/".to_string());
            format!("Sky: {}", place_label)
        } else {
            "Graph Viewer".to_string()
        };

        if place_id.is_none() {
            place_id = Some(userland::simple_uuid(b"/"));
        }

        let window_fields = userland::graph::Window {
            id: Uuid::nil(),
            title,
            x: 0,
            y: 0,
            width,
            height,
            z: 0,
            visible: true,
            target: None,
            active: false,
            is_root: true,
            is_place_root: place_id.is_some(),
            place_id: place_id,
            mode_index: mode_index,
            window_rect: None,
            gap: None,
            flex_direction: None,
            justify_content: None,
            align_items: None,
            tile_mode: Some(true),
        };
        let window = ctx.create_window_with(window_fields);
        
        // Set wallpaper using AppContext
        ctx.draw_bitmap(&window, WALLPAPER);

        let mut app = GraphViewerApp {
            window: window.clone(),
            place_id,
            root_widget: window.window_id(), // Use window as root parent
            widgets: BTreeMap::new(),
            contents: BTreeSet::new(),
            counter: 0,
        };
        
        app.refresh_contents();

        // Watch everything
        ctx.watch_graph(ThingFilter {
            kind: None,
            id: None,
        });

        app
    }

    fn on_event(&mut self, _ctx: &mut AppContext<'_>, ev: AppEvent) {
        match ev {
            AppEvent::Thing { thing, .. } => {
                let mut parent_match = false;
                if let Some(Value::Uuid(parent)) = thing.fields.get(&canon::PARENT) {
                    if Some(*parent) == self.place_id {
                        parent_match = true;
                    }
                }

                if parent_match {
                    self.contents.insert(thing.id);
                    self.update_widget(thing.id, &thing);
                } else if self.contents.contains(&thing.id) {
                    if let Some(Value::Uuid(parent)) = thing.fields.get(&canon::PARENT) {
                         if Some(*parent) != self.place_id {
                            self.remove_widget(thing.id);
                            return;
                        }
                    }
                    self.update_widget(thing.id, &thing);
                }

                if let Some(is_down) = thing
                    .fields
                    .get(&canon::MOUSE_DOWN)
                    .and_then(|v| v.as_bool())
                {
                    if is_down {
                        if self
                            .widgets
                            .values()
                            .any(|&widget_id| widget_id == thing.id)
                        {
                            self.handle_widget_click(thing.id);

                            let mut updates = graph::map();
                            updates.insert(canon::MOUSE_DOWN, Value::Bool(false));
                            graph::fiat(Some(thing.id), canon::WIDGET, updates);
                        }
                    }
                }
            }
            AppEvent::Edge { edge, .. } => {
                // If we see an edge involving our contents, we might want to draw it.
                // For now, let's just refresh if we see relevant edges.
                if self.contents.contains(&edge.src) && self.contents.contains(&edge.dst) {
                     self.create_edge_widget(edge.src, edge.dst, &edge.pred);
                }
            }
        }
    }

    fn tick(&mut self, _ctx: &mut AppContext<'_>, _tick: u64) {}
}

impl GraphViewerApp {
    fn discover_launch_intent() -> Option<(Uuid, Option<u8>, Uuid)> {
        let mut pattern = NodePattern::default();
        pattern.labels.push(canon::LAUNCH_INTENT);
        let intents = userland::graph::get_nodes(pattern);

        for intent in intents {
            if let Some(app) = intent.fields.get(&canon::APP).and_then(|v| v.as_text()) {
                if app == "graph_viewer" {
                    let place_id = intent.fields.get(&canon::PLACE).and_then(|v| v.as_uuid());
                    let mode_index = intent
                        .fields
                        .get(&canon::MODE_INDEX)
                        .and_then(|v| v.as_i64())
                        .map(|v| v as u8);

                    if let Some(place) = place_id {
                        return Some((place, mode_index, intent.id));
                    }
                }
            }
        }
        None
    }

    fn refresh_contents(&mut self) {
        if let Some(place_id) = self.place_id {
            let mut pattern = NodePattern::default();
            pattern.props.insert(canon::PARENT, Value::Uuid(place_id));

            let things = userland::graph::get_nodes(pattern);
            for thing in things {
                self.contents.insert(thing.id);
                self.update_widget(thing.id, &thing);
            }
            
            // Note: We cannot query existing edges easily via current API.
            // Edges will appear as AppEvent::Edge events are processed.
        }
    }

    fn remove_widget(&mut self, thing_id: Uuid) {
        self.contents.remove(&thing_id);
        if let Some(widget_id) = self.widgets.remove(&thing_id) {
            let mut fields = graph::map();
            fields.insert(canon::VISIBLE, Value::Bool(false));
            graph::fiat(Some(widget_id), canon::WIDGET, fields);
        }
    }

    fn update_widget(&mut self, thing_id: Uuid, thing: &GraphThing) {
        let widget_id = *self
            .widgets
            .entry(thing_id)
            // Use window ID as namespace for stability
            .or_insert_with(|| Uuid::new_v5(&self.root_widget, thing_id.as_bytes()));

        let mut fields = graph::map();
        fields.insert(canon::KIND, Value::Symbol(canon::WIDGET));
        // Parent directly to window (self.root_widget is now window_id)
        fields.insert(canon::PARENT, Value::Uuid(self.root_widget));
        fields.insert(canon::VISIBLE, Value::Bool(true));
        fields.insert(canon::cc('R', 'L'), Value::Text("thing_tile".to_string()));
        fields.insert(canon::WIDTH, Value::U64(64));
        fields.insert(canon::HEIGHT, Value::U64(80));

        // Label
        let label = thing
            .fields
            .get(&canon::NAME)
            .or_else(|| thing.fields.get(&canon::TITLE))
            .or_else(|| thing.fields.get(&canon::TEXT))
            .and_then(|v| match v {
                Value::Text(s) => Some(s.clone()),
                _ => None,
            })
            .unwrap_or_else(|| format!("{:?}", thing.kind));
        fields.insert(canon::TEXT, Value::Text(label.clone()));
        fields.insert(canon::LABEL, Value::Text(label)); // Ensure widget manager sees it

        // Icon: Assign based on thing kind if not explicitly set
        let icon_sym = canon::canon(b'I', b'C', b'N');
        let icon_name = if let Some(icon) = thing.fields.get(&icon_sym).and_then(|v| v.as_text()) {
            // Explicit icon set
            icon.to_string()
        } else {
            // Derive from kind
            match thing.kind {
                k if k == canon::FILE => {
                    // Check MIME type for more specific icons
                    if let Some(Value::Text(mime)) = thing.fields.get(&canon::MIME) {
                        if mime.starts_with("image/") {
                            "file".to_string() // Could use image-specific icon
                        } else {
                            "file".to_string()
                        }
                    } else {
                        "file".to_string()
                    }
                },
                k if k == canon::DIRECTORY => "folder".to_string(),
                k if k == canon::PLACE => "home".to_string(),
                k if k == canon::APP => "terminal".to_string(), // Apps get terminal icon
                _ => "file".to_string(), // Default fallback
            }
        };
        fields.insert(canon::ICON, Value::Text(icon_name));

        // Position: Use X/Y if available, else derive from ID hash for stability
        // Casting to U64 is correct for canon::X/Y based on ui_graph.rs
        let x = thing.fields.get(&canon::X).and_then(|v| v.as_u64()).unwrap_or_else(|| {
             (thing.id.as_u128() % 900 + 50) as u64
        });
        let y = thing.fields.get(&canon::Y).and_then(|v| v.as_u64()).unwrap_or_else(|| {
             (thing.id.as_u128() / 900 % 600 + 50) as u64
        });
        
        fields.insert(canon::X, Value::U64(x));
        fields.insert(canon::Y, Value::U64(y));

        graph::fiat(Some(widget_id), canon::WIDGET, fields);
    }
    
    fn create_edge_widget(&mut self, src: Uuid, dst: Uuid, label: &str) {
        // We create a widget representing the edge.
        // It's not keyed by a single thing ID, so we combine names.
        let seed = format!("{}-{}-{}", src, dst, label);
        let widget_id = Uuid::new_v5(&self.root_widget, seed.as_bytes());
        
        // We need coordinates of src and dst.
        // We can query the widgets for them if we updated them.
        // But we might not have updated them yet.
        // For now, let's just create it and let widget manager figure it out? No, widget manager is dumb.
        // query src thing:
        let src_thing = userland::graph::get_thing(src);
        let dst_thing = userland::graph::get_thing(dst);
        
        if let (Some(s), Some(d)) = (src_thing, dst_thing) {
             let sx = s.fields.get(&canon::X).and_then(|v| v.as_u64()).unwrap_or((src.as_u128() % 900 + 50) as u64);
             let sy = s.fields.get(&canon::Y).and_then(|v| v.as_u64()).unwrap_or((src.as_u128() / 900 % 600 + 50) as u64);
             let dx = d.fields.get(&canon::X).and_then(|v| v.as_u64()).unwrap_or((dst.as_u128() % 900 + 50) as u64);
             let dy = d.fields.get(&canon::Y).and_then(|v| v.as_u64()).unwrap_or((dst.as_u128() / 900 % 600 + 50) as u64);
             
             // Connector starts at center of src
             let start_x = sx + 32;
             let start_y = sy + 40;
             let end_x = dx + 32;
             let end_y = dy + 40;
             
             let mut fields = graph::map();
             fields.insert(canon::KIND, Value::Symbol(canon::WIDGET));
             fields.insert(canon::PARENT, Value::Uuid(self.root_widget));
             fields.insert(canon::VISIBLE, Value::Bool(true));
             fields.insert(canon::ROLE, Value::Text("connector".to_string()));
             fields.insert(canon::X, Value::U64(start_x));
             fields.insert(canon::Y, Value::U64(start_y));
             fields.insert(canon::WIDTH, Value::U64((end_x as i64 - start_x as i64) as u64)); // Hack: delta X
             fields.insert(canon::HEIGHT, Value::U64((end_y as i64 - start_y as i64) as u64)); // Hack: delta Y
             fields.insert(canon::LABEL, Value::Text(label.to_string()));
             
             graph::fiat(Some(widget_id), canon::WIDGET, fields);
        }
    }

    /// Handles a click on a widget.
    /// `widget_id` is the ID of the widget node in the graph.
    /// We look up the underlying `thing_id` (the content being displayed) via the `self.widgets` map.
    fn handle_widget_click(&mut self, widget_id: Uuid) {
        let thing_id = self
            .widgets
            .iter()
            .find(|(_, &w)| w == widget_id)
            .map(|(t, _)| *t);
        if let Some(thing_id) = thing_id {
            if let Some(thing) = userland::graph::get_thing(thing_id) {
                if thing.kind == canon::PLACE {
                    self.launch_place(thing_id);
                } else {
                    let app_name = thing
                        .fields
                        .get(&canon::APP)
                        .or_else(|| thing.fields.get(&canon::BIN_NAME))
                        .and_then(|v| match v {
                            Value::Text(s) => Some(s),
                            _ => None,
                        });

                    if let Some(app) = app_name {
                        userland::sys::spawn(app);
                    } else {
                        userland::sys::spawn("thing_viewer");
                    }
                }
            }
        }
    }

    fn launch_place(&mut self, place_id: Uuid) {
        self.counter += 1;
        let intent_id = Uuid::new_v5(&self.root_widget, &self.counter.to_le_bytes());
        let mut fields = graph::map();
        fields.insert(canon::KIND, Value::Symbol(canon::LAUNCH_INTENT));
        fields.insert(canon::APP, Value::Text("graph_viewer".to_string()));
        fields.insert(canon::PLACE, Value::Uuid(place_id));

        graph::fiat(Some(intent_id), canon::LAUNCH_INTENT, fields);
        userland::sys::spawn("graph_viewer");
    }
}
