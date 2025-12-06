#![cfg_attr(not(feature = "std"), no_std)]

extern crate alloc;

use alloc::collections::{BTreeMap, BTreeSet};
use alloc::format;
use alloc::string::ToString;
use thing_abi::GraphThing;
use userland::flex::{AlignItems, FlexDirection, FlexWrap, JustifyContent};
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

        let mut window_fields = userland::graph::Window {
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
        };
        let window = ctx.create_window_with(window_fields);

        // Create root widget
        let root_widget = Uuid::new_v5(&Uuid::NAMESPACE_OID, b"graph_viewer_root");
        let mut root_fields = graph::map();
        root_fields.insert(canon::KIND, Value::Symbol(canon::WIDGET));
        root_fields.insert(canon::PARENT, Value::Uuid(window.window_id()));
        root_fields.insert(canon::VISIBLE, Value::Bool(true));
        root_fields.insert(canon::cc('R', 'L'), Value::Text("window_root".to_string()));
        // Use Row layout with Wrap for icons
        root_fields.insert(canon::cc('F', 'D'), FlexDirection::Row.to_value());
        root_fields.insert(canon::cc('F', 'W'), FlexWrap::Wrap.to_value());
        root_fields.insert(canon::cc('J', 'C'), JustifyContent::Start.to_value());
        root_fields.insert(canon::cc('A', 'I'), AlignItems::Start.to_value());
        root_fields.insert(canon::cc('G', 'P'), Value::I64(10)); // Gap

        graph::fiat(Some(root_widget), canon::WIDGET, root_fields);

        let app = GraphViewerApp {
            window: window.clone(),
            place_id,
            root_widget,
            widgets: BTreeMap::new(),
            contents: BTreeSet::new(),
            counter: 0,
        };

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
                    // If we are tracking it, but parent doesn't match...
                    if let Some(Value::Uuid(parent)) = thing.fields.get(&canon::PARENT) {
                        // If it has a parent and it's not us, it moved.
                        if Some(*parent) != self.place_id {
                            self.remove_widget(thing.id);
                            return;
                        }
                    }
                    // If PARENT is missing, we assume it might be kept via edge, so we update.
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
                if let Some(place_id) = self.place_id {
                    if edge.src == place_id && edge.pred == "contains" {
                        // TODO: Handle edge deletion if ABI supports it.
                        // For now we only handle addition.
                        self.contents.insert(edge.dst);
                        if let Some(thing) = userland::graph::get_thing(edge.dst) {
                            self.update_widget(edge.dst, &thing);
                        }
                    }
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
            .or_insert_with(|| Uuid::new_v5(&self.root_widget, thing_id.as_bytes()));

        let mut fields = graph::map();
        fields.insert(canon::KIND, Value::Symbol(canon::WIDGET));
        fields.insert(canon::PARENT, Value::Uuid(self.root_widget));
        fields.insert(canon::VISIBLE, Value::Bool(true));
        fields.insert(canon::cc('R', 'L'), Value::Text("thing_tile".to_string()));
        fields.insert(canon::WIDTH, Value::U64(80));
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
        fields.insert(canon::TEXT, Value::Text(label));

        // Icon
        let icon_sym = canon::canon(b'I', b'C', b'N');
        if let Some(icon) = thing.fields.get(&icon_sym) {
            fields.insert(icon_sym, icon.clone());
        }

        // Position (if present)
        if let Some(x) = thing.fields.get(&canon::X) {
            fields.insert(canon::X, x.clone());
        }
        if let Some(y) = thing.fields.get(&canon::Y) {
            fields.insert(canon::Y, y.clone());
        }

        graph::fiat(Some(widget_id), canon::WIDGET, fields);
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
