use alloc::collections::{BTreeMap, VecDeque};
use alloc::format;
use alloc::string::String;
use alloc::string::ToString;
use alloc::vec;
use userland::prelude::*;
use userland::{canon, graph, AppEvent, Symbol, ThingFilter};
use uuid::Uuid;

const SMOKE_KIND: Symbol = canon::canon(b'G', b'S', b'M');
const NODE_NAME: &str = "graph-smoke-node";
const MAX_KEY_LOG: usize = 24;

pub struct DemoApp {
    _windows: BTreeMap<Uuid, WindowHandle>,
    selection_node: Uuid,
    top_bar_id: Uuid,
    _toolbar_id: Uuid,
    _list_id: Uuid,
    main_widget_id: Uuid,
    inspector_id: Uuid,
    status_id: Uuid,
    entries: &'static [DashboardEntry],
    key_count: usize,
    key_log: VecDeque<String>,
}

struct LayoutNumbers {
    gap: i32,
    top_bar_h: u64,
    rail_w: u64,
    sidebar_w: u64,
    mini_graph_h: u64,
    status_h: u64,
}

struct DashboardEntry {
    label: &'static str,
    target: &'static str,
    icon: &'static str,
    color: (u8, u8, u8),
}

const DASHBOARD_ENTRIES: &[DashboardEntry] = &[
    DashboardEntry {
        label: "Text Editor",
        target: "text_editor",
        icon: "menu",
        color: (0x35, 0x7d, 0xf5),
    },
    DashboardEntry {
        label: "Graph Viewer",
        target: "graph_viewer",
        icon: "home",
        color: (0x5c, 0xc1, 0x98),
    },
    DashboardEntry {
        label: "Thing Viewer",
        target: "thing_viewer",
        icon: "settings",
        color: (0xe7, 0x9c, 0x41),
    },
    DashboardEntry {
        label: "Self Edit",
        target: "self_editing_demo",
        icon: "arrow-back",
        color: (0xcc, 0x4a, 0x4a),
    },
];

impl App for DemoApp {
    fn init(ctx: &mut AppContext<'_>) -> Self {
        // Basic sizing for the dashboard layout; flex_grow values stretch to fill.
        let sizes = LayoutNumbers {
            gap: 8,
            top_bar_h: 48,
            rail_w: 140,
            sidebar_w: 340,
            mini_graph_h: 200,
            status_h: 120,
        };

        let widget_host_bundle = Uuid::new_v5(&Uuid::NAMESPACE_OID, b"widget_host");
        let mut windows = BTreeMap::new();

        let window_fields = userland::graph::Window {
            id: Uuid::nil(),
            title: "ThingOS Demo Dashboard".to_string(),
            x: 0,
            y: 0,
            width: 1280,
            height: 800,
            z: 0,
            visible: true,
            target: None,
            active: false,
            is_root: true,
            mode_index: Some(1), // F2
            window_rect: None,
            gap: Some(sizes.gap),
            flex_direction: Some("column".into()),
            justify_content: Some("start".into()),
            align_items: Some("stretch".into()),
        };
        let window = ctx.create_window_with(window_fields);

        // Top bar (fixed height)
        let top_bar_id = create_widget(
            "top_bar",
            "top_status_bar",
            &window,
            None,
            &[
                (canon::HEIGHT, Value::U64(sizes.top_bar_h)),
                (canon::cc('F', 'G'), Value::I64(0)),
                (canon::cc('F', 'S'), Value::I64(0)),
                (canon::TEXT, Value::Text("ThingOS Demo Dashboard".into())),
            ],
            widget_host_bundle,
        );

        // Toolbar row
        let toolbar_id = create_container(
            "toolbar_row",
            &window,
            None,
            &[
                (canon::ROLE, Value::Text("window_root".into())),
                (canon::cc('F', 'D'), Value::Text("row".into())),
                (canon::GAP, Value::I64((sizes.gap / 2).into())),
                (canon::HEIGHT, Value::U64(40)),
                (canon::cc('F', 'G'), Value::I64(0)),
                (canon::cc('F', 'S'), Value::I64(0)),
            ],
        );
        for entry in DASHBOARD_ENTRIES {
            create_widget(
                entry.label,
                "toolbar_button",
                &window,
                Some(toolbar_id),
                &[
                    (canon::TEXT, Value::Text(entry.label.into())),
                    (canon::TARGET, Value::Text(entry.target.into())),
                    (canon::ICON_NAME, Value::Text(entry.icon.into())),
                    (canon::canon(b'S', b'H', b'L'), Value::Bool(true)),
                ],
                widget_host_bundle,
            );
        }

        // Selection node for list binding
        let selection_node = Uuid::new_v5(&Uuid::NAMESPACE_OID, b"demo_selection");
        let mut selection_fields = graph::map();
        selection_fields.insert(canon::SELECTED_INDEX, Value::I64(0));
        graph::fiat(Some(selection_node), canon::WIDGET, selection_fields);

        // Body container (row: rail | main | sidebar)
        let body_id = create_container(
            "body_container",
            &window,
            None,
            &[
                (canon::ROLE, Value::Text("window_root".into())),
                (canon::cc('F', 'D'), Value::Text("row".into())),
                (canon::GAP, Value::I64(sizes.gap.into())),
                (canon::cc('F', 'G'), Value::I64(1)),
                (canon::cc('F', 'S'), Value::I64(1)),
            ],
        );

        // Left rail
        let list_id = create_widget(
            "left_rail",
            "listbox_default",
            &window,
            Some(body_id),
            &[
                (canon::WIDTH, Value::U64(sizes.rail_w)),
                (canon::cc('F', 'G'), Value::I64(0)),
                (canon::cc('F', 'S'), Value::I64(0)),
                (canon::BINDS, Value::Uuid(selection_node)),
            ],
            widget_host_bundle,
        );
        populate_launcher_items(list_id, selection_node, widget_host_bundle);

        // Main content (flex grow)
        let main_widget_id = create_widget(
            "main_content",
            "image",
            &window,
            Some(body_id),
            &[
                (canon::cc('F', 'G'), Value::I64(1)),
                (canon::cc('F', 'S'), Value::I64(1)),
                (canon::WIDTH, Value::U64(400)),
                (canon::HEIGHT, Value::U64(400)),
                (canon::cc('I', 'D'), Value::Bytes(create_demo_image_data())),
            ],
            widget_host_bundle,
        );

        // Right sidebar container (column)
        let sidebar_id = create_container(
            "sidebar_container",
            &window,
            Some(body_id),
            &[
                (canon::ROLE, Value::Text("window_root".into())),
                (canon::cc('F', 'D'), Value::Text("column".into())),
                (canon::GAP, Value::I64(sizes.gap.into())),
                (canon::WIDTH, Value::U64(sizes.sidebar_w)),
                (canon::cc('F', 'G'), Value::I64(0)),
                (canon::cc('F', 'S'), Value::I64(0)),
            ],
        );

        // Graph mini view
        create_widget(
            "mini_graph",
            "graph_mini_viewer",
            &window,
            Some(sidebar_id),
            &[
                (canon::HEIGHT, Value::U64(sizes.mini_graph_h)),
                (canon::cc('F', 'G'), Value::I64(0)),
                (canon::cc('F', 'S'), Value::I64(0)),
            ],
            widget_host_bundle,
        );

        // Inspector (fills remaining space)
        let inspector_id = create_widget(
            "inspector",
            "thing_inspector",
            &window,
            Some(sidebar_id),
            &[
                (canon::HEIGHT, Value::U64(260)),
                (canon::cc('F', 'G'), Value::I64(1)),
                (canon::cc('F', 'S'), Value::I64(1)),
            ],
            widget_host_bundle,
        );

        // Status widget (fixed height)
        let status_id = create_widget(
            "status_widget",
            "status_widget",
            &window,
            Some(sidebar_id),
            &[
                (canon::HEIGHT, Value::U64(sizes.status_h)),
                (canon::cc('F', 'G'), Value::I64(0)),
                (canon::cc('F', 'S'), Value::I64(0)),
            ],
            widget_host_bundle,
        );

        windows.insert(window.window_id(), window);

        let entries = DASHBOARD_ENTRIES;
        let mut app = DemoApp {
            _windows: windows,
            selection_node,
            top_bar_id,
            _toolbar_id: toolbar_id,
            _list_id: list_id,
            main_widget_id,
            inspector_id,
            status_id,
            entries,
            key_count: 0,
            key_log: VecDeque::new(),
        };

        app.update_selection(ctx, 0);

        ctx.watch_graph(ThingFilter {
            kind: Some(canon::KEY_PRESSED),
            id: None,
        });
        ctx.watch_graph(ThingFilter {
            kind: Some(canon::KEY_EVENT),
            id: None,
        });
        ctx.watch_graph(ThingFilter {
            kind: None,
            id: Some(selection_node),
        });

        app
    }

    fn on_event(&mut self, ctx: &mut AppContext<'_>, ev: AppEvent) {
        if let AppEvent::Thing { thing, .. } = ev {
            if thing.kind == canon::KEY_PRESSED {
                self.key_count += 1;
            }
            if thing.kind == canon::KEY_EVENT {
                self.record_key_event(&thing);
            }

            if thing.id == self.selection_node {
                if let Some(Value::I64(idx)) = thing.fields.get(&canon::SELECTED_INDEX) {
                    let idx = (*idx).max(0) as usize;
                    if idx < self.entries.len() {
                        self.update_selection(ctx, idx);
                    }
                }
            }
        }
    }

    fn tick(&mut self, _ctx: &mut AppContext<'_>, _tick: u64) {
        // No-op for now
    }
}

impl DemoApp {
    fn record_key_event(&mut self, thing: &graph::GraphThing) {
        let key_text = thing
            .fields
            .get(&canon::TEXT)
            .and_then(|v| v.as_text())
            .map(|s| s.to_string());

        if let Some(text) = key_text {
            if self.key_log.len() >= MAX_KEY_LOG {
                self.key_log.pop_front();
            }
            self.key_log.push_back(text);
        }
    }

    fn update_selection(&mut self, ctx: &mut AppContext<'_>, idx: usize) {
        let _ = ctx;
        let entry = &self.entries[idx];

        // Update top bar title
        let mut title_fields = graph::map();
        title_fields.insert(
            canon::TEXT,
            Value::Text(format!("ThingOS Demo Dashboard — {}", entry.label)),
        );
        graph::fiat(Some(self.top_bar_id), canon::WIDGET, title_fields);

        // Update inspector display
        let mut inspector_fields = graph::map();
        inspector_fields.insert(
            canon::TEXT,
            Value::Text(format!("selection:{}", entry.target)),
        );
        inspector_fields.insert(canon::KIND, Value::Text(entry.target.into()));
        inspector_fields.insert(canon::TITLE, Value::Text(entry.label.into()));
        graph::fiat(Some(self.inspector_id), canon::WIDGET, inspector_fields);

        // Update status text
        let mut status_fields = graph::map();
        status_fields.insert(
            canon::TEXT,
            Value::Text(format!("Selected {}", entry.label)),
        );
        graph::fiat(Some(self.status_id), canon::WIDGET, status_fields);

        // Update main content image with a color keyed to the entry
        let (r, g, b) = entry.color;
        let img = build_solid_image(320, 240, r, g, b);
        let mut main_fields = graph::map();
        main_fields.insert(canon::cc('I', 'D'), Value::Bytes(img));
        main_fields.insert(canon::WIDTH, Value::U64(320));
        main_fields.insert(canon::HEIGHT, Value::U64(240));
        graph::fiat(Some(self.main_widget_id), canon::WIDGET, main_fields);

        // Keep selection node in sync (useful if we programmatically change idx later)
        let mut selection_fields = graph::map();
        selection_fields.insert(canon::SELECTED_INDEX, Value::I64(idx as i64));
        graph::fiat(Some(self.selection_node), canon::WIDGET, selection_fields);

        // Optional: request launch of the selected package
        let mut launch_fields = graph::map();
        launch_fields.insert(canon::PACKAGE, Value::Text(entry.target.into()));
        launch_fields.insert(canon::NAME, Value::Text(entry.label.into()));
        graph::fiat(None, canon::LAUNCH_REQUEST, launch_fields);
    }
}

fn create_container(
    name: &str,
    window: &WindowHandle,
    parent: Option<Uuid>,
    extra_fields: &[(canon::Symbol, Value)],
) -> Uuid {
    let widget_id = Uuid::new_v5(&Uuid::NAMESPACE_OID, name.as_bytes());
    let mut fields = graph::map();
    fields.insert(canon::KIND, Value::Symbol(canon::WIDGET));
    fields.insert(
        canon::PARENT,
        Value::Uuid(parent.unwrap_or_else(|| window.window_id())),
    );
    fields.insert(canon::VISIBLE, Value::Bool(true));
    for (k, v) in extra_fields {
        fields.insert(*k, v.clone());
    }
    graph::fiat(Some(widget_id), canon::WIDGET, fields);
    graph::that(window.window_id(), "contains", widget_id, 0);
    widget_id
}

fn create_widget(
    name: &str,
    kind: &str,
    window: &WindowHandle,
    parent: Option<Uuid>,
    extra_fields: &[(canon::Symbol, Value)],
    widget_host_bundle: Uuid,
) -> Uuid {
    let widget_id = Uuid::new_v5(&Uuid::NAMESPACE_OID, name.as_bytes());
    let mut fields = graph::map();
    fields.insert(canon::KIND, Value::Symbol(canon::WIDGET));
    fields.insert(canon::cc('W', 'K'), Value::Text(String::from(kind)));
    fields.insert(
        canon::PARENT,
        Value::Uuid(parent.unwrap_or_else(|| window.window_id())),
    );
    fields.insert(canon::VISIBLE, Value::Bool(true));

    for (k, v) in extra_fields {
        fields.insert(*k, v.clone());
    }

    graph::fiat(Some(widget_id), canon::WIDGET, fields);
    graph::grant_capability(widget_host_bundle, widget_id, "CAN_READ");
    graph::that(window.window_id(), "contains", widget_id, 0);
    widget_id
}

fn populate_launcher_items(list_id: Uuid, selection_node: Uuid, widget_host_bundle: Uuid) {
    for (i, entry) in DASHBOARD_ENTRIES.iter().enumerate() {
        let item_id = Uuid::new_v5(&Uuid::NAMESPACE_OID, entry.label.as_bytes());
        let mut fields = graph::map();
        fields.insert(canon::KIND, Value::Symbol(canon::ITEM));
        fields.insert(canon::PARENT, Value::Uuid(list_id));
        fields.insert(canon::ITEM_LABEL, Value::Text(entry.label.into()));
        fields.insert(canon::ITEM_VALUE, Value::Text(entry.target.into()));
        fields.insert(canon::ICON_NAME, Value::Text(entry.icon.into()));
        fields.insert(canon::VISIBLE, Value::Bool(true));
        graph::fiat(Some(item_id), canon::ITEM, fields);

        // Allow the widget host to read items
        graph::grant_capability(widget_host_bundle, item_id, "CAN_READ");

        if i == 0 {
            let mut selection_fields = graph::map();
            selection_fields.insert(canon::SELECTED_INDEX, Value::I64(0));
            graph::fiat(Some(selection_node), canon::WIDGET, selection_fields);
        }
    }
}

fn create_demo_image_data() -> alloc::vec::Vec<u8> {
    let mut data = vec![0; 16 * 16 * 4];
    for i in 0..16 * 16 {
        data[i * 4] = 255;
        data[i * 4 + 1] = 0;
        data[i * 4 + 2] = 0;
        data[i * 4 + 3] = 255;
    }
    data
}

fn build_solid_image(width: u32, height: u32, r: u8, g: u8, b: u8) -> alloc::vec::Vec<u8> {
    let mut data = vec![0; (width * height * 4) as usize];
    for y in 0..height {
        for x in 0..width {
            let idx = ((y * width + x) * 4) as usize;
            data[idx] = r;
            data[idx + 1] = g;
            data[idx + 2] = b;
            data[idx + 3] = 0xFF;
        }
    }
    data
}
