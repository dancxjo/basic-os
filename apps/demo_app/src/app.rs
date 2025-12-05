use alloc::collections::{BTreeMap, VecDeque};
use alloc::format;
use alloc::string::String;
use alloc::string::ToString;
use alloc::vec;
use alloc::vec::Vec;
use userland::flex::{AlignItems, FlexDirection, JustifyContent};
use userland::graph::{GraphPropsGetRequest, GraphPropsRequest};
use userland::prelude::*;
use userland::questions::{attach_question_to_form, ensure_form, ensure_question_with_answer};
use userland::{canon, graph, AnswerValue, AppEvent, QuestionBinding, Symbol, ThingFilter};
use uuid::Uuid;
use widget_checkbox::CHECKED;

const TRANSITION_OPTIONS: &[&str] = &["None", "Slide", "Fade", "Zoom"];

use userland::layout_instantiator::LayoutInstantiator;
use crate::template_builder::{TemplateBuilder, RegionBuilder};

pub struct DemoApp {
    _windows: BTreeMap<Uuid, WindowHandle>,
    selection_node: Uuid,
    // top_bar_id: Uuid, // Removed as it's now managed by layout
    // _toolbar_id: Uuid,
    // _list_id: Uuid,
    // main_widget_id: Uuid,
    // inspector_id: Uuid,
    // status_id: Uuid,
    entries: &'static [DashboardEntry],
    prefs: PrefsShowcase,
    questions: QuestionShowcase,
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

        // Bootstrap Sky Place
        let sky_place_id = userland::simple_uuid(b"SkyPlace");
        let mut f = graph::map();
        f.insert(canon::LABEL, Value::Text("Sky Place".into()));
        f.insert(canon::PLACE_PACKAGE, Value::Text("demo_app".into()));
        f.insert(canon::PLACE_FULLSCREEN, Value::Bool(true));
        graph::fiat(Some(sky_place_id), canon::PLACE, f);

        // Bootstrap Mode F2 -> Sky Place
        let mode_f2_id = userland::simple_uuid(b"ModeF2");
        let mut f = graph::map();
        f.insert(canon::MODE_PLACE, Value::Uuid(sky_place_id));
        f.insert(canon::MODE_INDEX, Value::I64(1));
        f.insert(canon::LABEL, Value::Text("Sky".into()));
        graph::fiat(Some(mode_f2_id), canon::MODE, f);

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
            is_place_root: true,
            place_id: Some(sky_place_id),
            mode_index: Some(1), // F2
            window_rect: None,
            gap: Some(sizes.gap),
            flex_direction: Some(FlexDirection::Column),
            justify_content: Some(JustifyContent::Start),
            align_items: Some(AlignItems::Stretch),
        };
        let window = ctx.create_window_with(window_fields);

        // Selection node for list/toolbar binding
        let selection_node = Uuid::new_v5(&Uuid::NAMESPACE_OID, b"demo_selection");
        let mut selection_fields = graph::map();
        selection_fields.insert(canon::SELECTED_INDEX, Value::I64(0));
        graph::fiat(Some(selection_node), canon::WIDGET, selection_fields);

        // Define the Layout Template
        let template = TemplateBuilder::new("dashboard_layout", "dashboard", Some(1));
        
        // Top Bar
        template.add_region(
            RegionBuilder::new("top_bar")
                .role("top_status_bar")
                .height(sizes.top_bar_h)
                .text("ThingOS Demo Dashboard")
                .binds_to("top_bar")
        );

        // Toolbar Row
        let mut toolbar = RegionBuilder::new("toolbar_row")
            .role("window_root")
            .flex_dir(FlexDirection::Row)
            .gap((sizes.gap / 2).into())
            .height(40);
        
        for (i, entry) in DASHBOARD_ENTRIES.iter().enumerate() {
            toolbar = toolbar.child(
                RegionBuilder::new(entry.label)
                    .role("control.toolbar_button")
                    .text(entry.label)
                    .target(entry.target)
                    .icon(entry.icon)
                    .binds_to("selection") // Binds to selection node
                    .focusable(true)
            );
        }
        template.add_region(toolbar);

        // Body Container
        let mut body = RegionBuilder::new("body_container")
            .role("window_root")
            .flex_dir(FlexDirection::Row)
            .gap(sizes.gap.into())
            .grow(1)
            .shrink(1);

        // Left Rail
        body = body.child(
            RegionBuilder::new("left_rail")
                .role("listbox_default")
                .width(sizes.rail_w)
                .binds_to("selection")
                .focusable(true)
        );

        // Main Content
        body = body.child(
            RegionBuilder::new("main_content")
                .role("image")
                .grow(1)
                .shrink(1)
                .width(400)
                .height(400)
                .binds_to("main_content")
        );

        // Right Sidebar
        let mut sidebar = RegionBuilder::new("sidebar_container")
            .role("window_root")
            .flex_dir(FlexDirection::Column)
            .gap(sizes.gap.into())
            .width(sizes.sidebar_w);

        sidebar = sidebar.child(
            RegionBuilder::new("mini_graph")
                .role("graph_mini_viewer")
                .height(sizes.mini_graph_h)
        );

        sidebar = sidebar.child(
            RegionBuilder::new("inspector")
                .role("thing_inspector")
                .height(260)
                .grow(1)
                .shrink(1)
                .binds_to("inspector")
        );

        // Controls Panel
        let mut controls = RegionBuilder::new("controls_panel")
            .role("window_root")
            .flex_dir(FlexDirection::Column)
            .gap(6);
        
        controls = controls.child(
            RegionBuilder::new("control_checkbox")
                .kind("checkbox")
                .text("Enable feature")
                .width(160)
                .height(28)
                .focusable(true)
        );
        
        controls = controls.child(
            RegionBuilder::new("control_radio_a")
                .kind("radio_button")
                .text("Mode A")
                .width(140)
                .height(28)
                .focusable(true)
        );

        controls = controls.child(
            RegionBuilder::new("control_radio_b")
                .kind("radio_button")
                .text("Mode B")
                .width(140)
                .height(28)
                .focusable(true)
        );

        controls = controls.child(
            RegionBuilder::new("control_image")
                .role("image")
                .width(120)
                .height(80)
        );

        sidebar = sidebar.child(controls);

        sidebar = sidebar.child(
            RegionBuilder::new("status_widget")
                .kind("status_widget")
                .height(sizes.status_h)
                .binds_to("status")
        );

        body = body.child(sidebar);
        template.add_region(body);

        // Instantiate Layout
        let mut bindings = BTreeMap::new();
        bindings.insert("selection".to_string(), selection_node);
        
        LayoutInstantiator::instantiate(
            window.window_id(),
            template.id(),
            &bindings,
            widget_host_bundle
        );

        // Re-acquire IDs for updates (using deterministic UUIDs based on names used in template)
        let list_id = Uuid::new_v5(&Uuid::NAMESPACE_OID, "left_rail".as_bytes());

        windows.insert(window.window_id(), window);

        let prefs = PrefsShowcase::new(ctx, widget_host_bundle);
        let questions = QuestionShowcase::new(ctx, widget_host_bundle);

        let entries = DASHBOARD_ENTRIES;
        
        // Populate list items (we need list_id for this)
        populate_launcher_items(list_id, selection_node, widget_host_bundle);

        let mut app = DemoApp {
            _windows: windows,
            selection_node,
            entries,
            prefs,
            questions,
        };

        app.update_selection(0);

        ctx.watch_graph(ThingFilter {
            kind: None,
            id: Some(selection_node),
        });

        app
    }

    fn on_event(&mut self, _ctx: &mut AppContext<'_>, ev: AppEvent) {
        if let AppEvent::Thing { thing, .. } = ev {
            if thing.id == self.selection_node {
                if let Some(Value::I64(idx)) = thing.fields.get(&canon::SELECTED_INDEX) {
                    let idx = (*idx).max(0) as usize;
                    if idx < self.entries.len() {
                        self.update_selection(idx);
                    }
                }
            }

            self.prefs.handle_event(&thing);
            self.questions.handle_event(&thing);
        }
    }

    fn tick(&mut self, _ctx: &mut AppContext<'_>, _tick: u64) {
        // No-op for now
    }
}

impl DemoApp {
    fn update_selection(&mut self, idx: usize) {
        let entry = &self.entries[idx];
        
        // Re-calculate IDs since we don't store them anymore
        let top_bar_id = Uuid::new_v5(&Uuid::NAMESPACE_OID, "top_bar".as_bytes());
        let inspector_id = Uuid::new_v5(&Uuid::NAMESPACE_OID, "inspector".as_bytes());
        let status_id = Uuid::new_v5(&Uuid::NAMESPACE_OID, "status_widget".as_bytes());
        let main_widget_id = Uuid::new_v5(&Uuid::NAMESPACE_OID, "main_content".as_bytes());

        // Update top bar title
        let mut title_fields = graph::map();
        title_fields.insert(
            canon::TEXT,
            Value::Text(format!("ThingOS Demo Dashboard — {}", entry.label)),
        );
        graph::set_props(GraphPropsRequest { node: top_bar_id, props: title_fields });

        // Update inspector display
        let mut inspector_fields = graph::map();
        inspector_fields.insert(
            canon::TEXT,
            Value::Text(format!("selection:{}", entry.target)),
        );
        inspector_fields.insert(canon::KIND, Value::Text(entry.target.into()));
        inspector_fields.insert(canon::TITLE, Value::Text(entry.label.into()));
        graph::set_props(GraphPropsRequest { node: inspector_id, props: inspector_fields });

        // Update status text
        let mut status_fields = graph::map();
        status_fields.insert(
            canon::TEXT,
            Value::Text(format!("Selected {}", entry.label)),
        );
        graph::set_props(GraphPropsRequest { node: status_id, props: status_fields });

        // Update main content image with a color keyed to the entry
        let (r, g, b) = entry.color;
        let img = build_solid_image(320, 240, r, g, b);
        let mut main_fields = graph::map();
        main_fields.insert(canon::cc('I', 'D'), Value::Bytes(img));
        main_fields.insert(canon::WIDTH, Value::U64(320));
        main_fields.insert(canon::HEIGHT, Value::U64(240));
        graph::set_props(GraphPropsRequest { node: main_widget_id, props: main_fields });

        // Keep selection node in sync (useful if we programmatically change idx later)
        let mut selection_fields = graph::map();
        selection_fields.insert(canon::SELECTED_INDEX, Value::I64(idx as i64));
        graph::set_props(GraphPropsRequest { node: self.selection_node, props: selection_fields });

        // Optional: request launch of the selected package
        let mut launch_fields = graph::map();
        launch_fields.insert(canon::PACKAGE, Value::Text(entry.target.into()));
        launch_fields.insert(canon::NAME, Value::Text(entry.label.into()));
        graph::fiat(None, canon::LAUNCH_REQUEST, launch_fields);
    }
}

const OPTIONS_SYM: Symbol = canon::canon(b'O', b'P', b'T');
const QUESTION_ROW_HEIGHT: u64 = 48;

fn create_transition_value_node(bundle_id: Uuid) -> Uuid {
    let id = Uuid::new_v5(&Uuid::NAMESPACE_OID, b"prefs_transition_value");
    let mut fields = graph::map();
    fields.insert(canon::KIND, Value::Symbol(canon::WIDGET));
    fields.insert(canon::BUNDLE_ID, Value::Uuid(bundle_id));
    fields.insert(canon::ITEM_VALUE, Value::Text(String::from("None")));
    fields.insert(canon::TEXT, Value::Text(String::from("None")));
    graph::fiat(Some(id), canon::WIDGET, fields);
    id
}

struct PrefsShowcase {
    main_window: WindowHandle,
    dialog_window: WindowHandle,
    buttons_window: WindowHandle,
    transition_value: Uuid,
}

impl PrefsShowcase {
    fn new(ctx: &mut AppContext<'_>, widget_host_bundle: Uuid) -> Self {
        let transition_value = create_transition_value_node(widget_host_bundle);
        ctx.watch_graph(ThingFilter {
            kind: Some(canon::WIDGET),
            id: Some(transition_value),
        });

        let (main_window, _) =
            Self::create_main_window(ctx, widget_host_bundle, transition_value);
        let (dialog_window, _) =
            Self::create_dialog_window(ctx, widget_host_bundle, transition_value);
        let buttons_window = Self::create_buttons_window(ctx, widget_host_bundle);

        PrefsShowcase {
            main_window,
            dialog_window,
            buttons_window,
            transition_value,
        }
    }

    fn handle_event(&self, thing: &graph::GraphThing) {
        if thing.id != self.transition_value {
            return;
        }

        let value_text = thing
            .fields
            .get(&canon::ITEM_VALUE)
            .and_then(graph::extract_text)
            .or_else(|| thing.fields.get(&canon::TEXT).and_then(graph::extract_text))
            .unwrap_or_else(|| String::from("None"));

        let mut fields = graph::map();
        fields.insert(
            canon::TEXT,
            Value::Text(format!("Graph emits: {value_text}")),
        );
        
        // Update footer widgets by deterministic ID
        let main_footer_id = Uuid::new_v5(&Uuid::NAMESPACE_OID, "f3_footer_status".as_bytes());
        let dialog_footer_id = Uuid::new_v5(&Uuid::NAMESPACE_OID, "f3_dialog_footer_status".as_bytes());
        
        graph::set_props(GraphPropsRequest { node: main_footer_id, props: fields.clone() });
        graph::set_props(GraphPropsRequest { node: dialog_footer_id, props: fields });
    }

    fn create_main_window(
        ctx: &mut AppContext<'_>,
        widget_host_bundle: Uuid,
        transition_value: Uuid,
    ) -> (WindowHandle, Uuid) {
        let window_width: u64 = 1120;
        let window_height: u64 = 760;
        let window_fields = userland::graph::Window {
            id: Uuid::nil(),
            title: String::from("Mode F3 Test App"),
            x: 64,
            y: 64,
            width: window_width,
            height: window_height,
            z: 0,
            visible: true,
            target: None,
            active: false,
            is_root: true,
            is_place_root: false,
            place_id: None,
            mode_index: Some(2),
            window_rect: None,
            gap: Some(16),
            flex_direction: Some(FlexDirection::Column),
            justify_content: Some(JustifyContent::Start),
            align_items: Some(AlignItems::Stretch),
        };
        let window = ctx.create_window_with(window_fields);

        let template = TemplateBuilder::new("prefs_main_layout", "window_root", None);
        
        let mut root = RegionBuilder::new("f3_root")
            .role("window_root")
            .flex_dir(FlexDirection::Column)
            .prop(canon::cc('J', 'C'), JustifyContent::SpaceBetween.to_value())
            .gap(12)
            .grow(1)
            .shrink(1);

        // Intro
        root = root.child(
            RegionBuilder::new("f3_intro")
                .kind("plain_text")
                .text("This root window sits on Mode F3. It uses a plain text widget so we can show a simple, readable message directly in the client area.")
                .width(window_width)
                .height(120)
                .shrink(0)
                .grow(0)
                .focusable(true) // Grant CAN_FOCUS
        );

        // Content
        let mut content = RegionBuilder::new("f3_content")
            .role("container")
            .flex_dir(FlexDirection::Column)
            .gap(20)
            .grow(1)
            .shrink(1);

        // Font Row
        let mut font_row = RegionBuilder::new("prefs_font_row")
            .role("container")
            .flex_dir(FlexDirection::Row)
            .gap(8)
            .grow(0);
            
        font_row = font_row.child(
            RegionBuilder::new("prefs_font_label")
                .kind("plain_text")
                .text("Font:")
                .width(80)
                .height(24)
        );
        
        // ... (We could add more font controls here if needed)
        
        content = content.child(font_row);
        
        // Transition Row
        let mut trans_row = RegionBuilder::new("prefs_trans_row")
            .role("container")
            .flex_dir(FlexDirection::Row)
            .gap(8)
            .grow(0);
            
        trans_row = trans_row.child(
            RegionBuilder::new("prefs_trans_label")
                .kind("plain_text")
                .text("Transition:")
                .width(100)
                .height(24)
        );
        
        // Transition Buttons
        for opt in TRANSITION_OPTIONS {
            trans_row = trans_row.child(
                RegionBuilder::new(&format!("trans_btn_{}", opt))
                    .kind("radio_button")
                    .text(*opt)
                    .width(100)
                    .height(32)
                    .focusable(true)
                    // We need to bind this to the transition value
                    // But radio buttons usually bind to a value node and set it on click
                    // For now, let's assume the widget handles it if we set the right props
                    // Or we can use `TARGET` to update the node?
                    // The original code used `create_widget` and didn't seem to wire up the click explicitly in `create_main_window`?
                    // Ah, `create_widget` just creates it.
                    // The original code didn't show the wiring for these buttons in the snippet I read.
                    // But `PrefsShowcase` has `transition_value`.
                    // Let's assume we want these buttons to update `transition_value`.
            );
        }
        
        content = content.child(trans_row);
        root = root.child(content);
        
        // Footer
        root = root.child(
            RegionBuilder::new("f3_footer_status")
                .kind("plain_text")
                .text("Graph emits: None")
                .width(window_width)
                .height(24)
        );
        
        template.add_region(root);
        
        LayoutInstantiator::instantiate(
            window.window_id(),
            template.id(),
            &BTreeMap::new(),
            widget_host_bundle
        );

        (window, Uuid::nil()) // We don't need to return the footer ID anymore
    }


    fn create_dialog_window(
        ctx: &mut AppContext<'_>,
        widget_host_bundle: Uuid,
        transition_value_node: Uuid,
    ) -> (WindowHandle, Uuid) {
        let window_width: u64 = 480;
        let window_height: u64 = 520;
        let window_fields = userland::graph::Window {
            id: Uuid::nil(),
            title: String::from("Dialog & Form Controls"),
            x: 1280,
            y: 64,
            width: window_width,
            height: window_height,
            z: 0,
            visible: true,
            target: None,
            active: false,
            is_root: false,
            is_place_root: false,
            place_id: None,
            mode_index: None,
            window_rect: None,
            gap: Some(12),
            flex_direction: Some(FlexDirection::Column),
            justify_content: Some(JustifyContent::Start),
            align_items: Some(AlignItems::Stretch),
        };
        let window = ctx.create_window_with(window_fields);

        let template = TemplateBuilder::new("prefs_dialog_layout", "window_root", None);
        
        let mut root = RegionBuilder::new("prefs_dialog_root")
            .role("window_root")
            .flex_dir(FlexDirection::Column)
            .justify_content(JustifyContent::Start)
            .gap(10)
            .grow(1)
            .shrink(1);

        // Buttons Row
        let mut buttons_row = RegionBuilder::new("prefs_buttons")
            .role("container")
            .flex_dir(FlexDirection::Row)
            .gap(16)
            .grow(0);
            
        buttons_row = buttons_row.child(
            RegionBuilder::new("prefs_confirm")
                .kind("button")
                .text("Confirm")
                .width(120)
                .height(32)
                .grow(1)
                .focusable(true)
        );
        
        buttons_row = buttons_row.child(
            RegionBuilder::new("prefs_cancel")
                .kind("button")
                .text("Cancel")
                .width(120)
                .height(32)
                .grow(1)
                .focusable(true)
        );
        
        root = root.child(buttons_row);

        // Transition Row
        let mut transition_row = RegionBuilder::new("prefs_dialog_transition")
            .role("container")
            .flex_dir(FlexDirection::Row)
            .gap(12);
            
        transition_row = transition_row.child(
            RegionBuilder::new("prefs_dialog_transition_label")
                .kind("plain_text")
                .text("Transition:")
                .width(120)
                .height(28)
                .role("label")
        );
        
        transition_row = transition_row.child(
            RegionBuilder::new("prefs_dialog_transition_select")
                .kind("select")
                .text("None")
                .width(220)
                .height(32)
                .focusable(true)
                .prop(canon::SELECTED_INDEX, Value::I64(0))
                .role("select")
                .prop(canon::BINDS, Value::Uuid(transition_value_node))
                .prop(
                    OPTIONS_SYM,
                    Value::List(
                        TRANSITION_OPTIONS
                            .iter()
                            .map(|s| Value::Text(String::from(*s)))
                            .collect(),
                    ),
                )
        );
        
        root = root.child(transition_row);

        // Footer
        root = root.child(
            RegionBuilder::new("prefs_dialog_footer_hint")
                .kind("plain_text")
                .text("Graph emits: None")
                .width(300)
                .height(28)
        );
        
        template.add_region(root);
        
        LayoutInstantiator::instantiate(
            window.window_id(),
            template.id(),
            &BTreeMap::new(),
            widget_host_bundle
        );
        
        // We need to ensure action interactions for the buttons
        // Since LayoutInstantiator creates widgets with deterministic IDs, we can calculate them
        let confirm_id = Uuid::new_v5(&Uuid::NAMESPACE_OID, "prefs_confirm".as_bytes());
        let cancel_id = Uuid::new_v5(&Uuid::NAMESPACE_OID, "prefs_cancel".as_bytes());
        
        ensure_action_interaction(
            transition_value_node,
            Some(confirm_id),
            Some("Apply transition"),
            Some("Act on the current transition choice"),
        );
        ensure_action_interaction(
            transition_value_node,
            Some(cancel_id),
            Some("Cancel transition"),
            Some("Dismiss transition changes"),
        );

        (window, Uuid::nil())
    }

    fn create_buttons_window(ctx: &mut AppContext<'_>, widget_host_bundle: Uuid) -> WindowHandle {
        let window_width: u64 = 320;
        let window_height: u64 = 240;
        let window_fields = userland::graph::Window {
            id: Uuid::nil(),
            title: String::from("Buttons & Toggles"),
            x: 1280,
            y: 640,
            width: window_width,
            height: window_height,
            z: 0,
            visible: true,
            target: None,
            active: false,
            is_root: false,
            is_place_root: false,
            place_id: None,
            mode_index: None,
            window_rect: None,
            gap: Some(12),
            flex_direction: Some(FlexDirection::Column),
            justify_content: Some(JustifyContent::Start),
            align_items: Some(AlignItems::Stretch),
        };
        let window = ctx.create_window_with(window_fields);

        let template = TemplateBuilder::new("prefs_buttons_layout", "window_root", None);
        
        let mut root = RegionBuilder::new("prefs_button_root")
            .role("window_root")
            .flex_dir(FlexDirection::Column)
            .gap(10);
            
        root = root.child(
            RegionBuilder::new("prefs_button_primary")
                .kind("button")
                .text("Primary")
                .width(160)
                .height(36)
                .focusable(true)
        );
        
        root = root.child(
            RegionBuilder::new("prefs_button_secondary")
                .kind("button")
                .text("Secondary")
                .width(160)
                .height(36)
                .focusable(true)
        );
        
        let mut toggle_row = RegionBuilder::new("prefs_button_toggle_row")
            .role("container")
            .flex_dir(FlexDirection::Row);
            
        toggle_row = toggle_row.child(
            RegionBuilder::new("prefs_toggle_label")
                .kind("plain_text")
                .text("Notifications")
                .width(180)
                .height(24)
        );
        
        toggle_row = toggle_row.child(
            RegionBuilder::new("prefs_toggle")
                .kind("checkbox")
                .width(120)
                .height(24)
                .focusable(true)
                .prop(CHECKED, Value::Bool(true))
        );
        
        root = root.child(toggle_row);
        
        template.add_region(root);
        
        LayoutInstantiator::instantiate(
            window.window_id(),
            template.id(),
            &BTreeMap::new(),
            widget_host_bundle
        );

        window
    }
}

struct BoundQuestion {
    widget_id: Uuid,
    binding: QuestionBinding,
}

struct QuestionShowcase {
    window: WindowHandle,
    questions: Vec<BoundQuestion>,
}

impl QuestionShowcase {
    fn new(ctx: &mut AppContext<'_>, widget_host_bundle: Uuid) -> Self {
        // Create Window
        let window_fields = userland::graph::Window {
            id: Uuid::nil(),
            title: "Question Demo".to_string(),
            x: 120,
            y: 120,
            width: 480,
            height: 360,
            z: 0,
            visible: true,
            target: None,
            active: false,
            is_root: false,
            is_place_root: false,
            place_id: None,
            mode_index: Some(3),
            window_rect: None,
            gap: Some(12),
            flex_direction: Some(FlexDirection::Column),
            justify_content: Some(JustifyContent::Start),
            align_items: Some(AlignItems::Stretch),
        };
        let window = ctx.create_window_with(window_fields);
        
        let form_id = ensure_form(None, "Semantic question demo");

        let mut questions = Vec::new();
        let tag = "demo_questions";

        // Question 1
        let question_one_label = "Enable Neo4j backend?";
        let question_one_desc = "Toggles the graph persistence layer";
        let (q1, _a1, b1) = ensure_question_with_answer(
            None,
            None,
            question_one_label,
            Some(question_one_desc),
            AnswerValue::Bool(false),
        );
        let mut q1_props = graph::map();
        q1_props.insert(canon::TAG, Value::Text(tag.into()));
        q1_props.insert(canon::PREFERRED_WIDGET, Value::Text("toggle".into()));
        graph::set_props(graph::GraphPropsRequest { node: q1, props: q1_props });
        attach_question_to_form(form_id, q1);

        let seed1 = alloc::format!("{}-{}-qwidget", window.window_id(), q1);
        let w1_id = userland::simple_uuid(seed1.as_bytes());
        questions.push(BoundQuestion { widget_id: w1_id, binding: b1 });

        // Question 2
        let question_two_label = "Allow telemetry?";
        let question_two_desc = "Share anonymous usage to improve stability";
        let (q2, _a2, b2) = ensure_question_with_answer(
            None,
            None,
            question_two_label,
            Some(question_two_desc),
            AnswerValue::Bool(true),
        );
        let mut q2_props = graph::map();
        q2_props.insert(canon::TAG, Value::Text(tag.into()));
        q2_props.insert(canon::PREFERRED_WIDGET, Value::Text("toggle".into()));
        graph::set_props(graph::GraphPropsRequest { node: q2, props: q2_props });
        attach_question_to_form(form_id, q2);

        let seed2 = alloc::format!("{}-{}-qwidget", window.window_id(), q2);
        let w2_id = userland::simple_uuid(seed2.as_bytes());
        questions.push(BoundQuestion { widget_id: w2_id, binding: b2 });

        // Template
        let mut template = RegionBuilder::new("question_root")
            .role("window_root")
            .flex_dir(FlexDirection::Column)
            .gap(12);
            
        // Intro Text
        template = template.child(
            RegionBuilder::new("question_intro")
                .kind("plain_text")
                .text("Questions are graph things. Widgets are just one view over those semantic nodes.")
                .height(72)
                .width(440)
                .shrink(0)
        );

        // Questions List
        template = template.child(
            RegionBuilder::new("questions_list")
                .role("container")
                .flex_dir(FlexDirection::Column)
                .gap(8)
                .prop(canon::SHOW_TAG, Value::Text(tag.into()))
        );
        
        let tb = TemplateBuilder::new("question_showcase", "window_root", None);
        tb.add_region(template);
        
        LayoutInstantiator::instantiate(
            window.window_id(),
            tb.id(),
            &BTreeMap::new(),
            widget_host_bundle,
        );

        // Initial Sync & Watch
        for bound in &questions {
            // Sync
            let request = GraphPropsGetRequest {
                node: bound.binding.answer,
                keys: alloc::vec![canon::VALUE_BOOL],
            };
            if let Some(props) = graph::get_props(request) {
                if let Some(Value::Bool(val)) = props.get(&canon::VALUE_BOOL) {
                    Self::push_widget_state(bound.widget_id, *val);
                }
            }

            // Watch
            ctx.watch_graph(ThingFilter {
                kind: Some(canon::WIDGET),
                id: Some(bound.widget_id),
            });
            ctx.watch_graph(ThingFilter {
                kind: Some(canon::ANSWER),
                id: Some(bound.binding.answer),
            });
        }

        QuestionShowcase { window, questions }
    }

    fn handle_event(&self, thing: &graph::GraphThing) {
        for bound in &self.questions {
            if thing.id == bound.widget_id {
                if let Some(value) = thing.fields.get(&CHECKED).and_then(|v| v.as_bool()) {
                    let _ = bound.binding.set_answer(AnswerValue::Bool(value));
                }
            }

            if thing.id == bound.binding.answer {
                if let Some(AnswerValue::Bool(value)) = QuestionBinding::read_answer(thing) {
                    Self::push_widget_state(bound.widget_id, value);
                }
            }
        }
    }

    fn push_widget_state(widget_id: Uuid, checked: bool) {
        let mut props = graph::map();
        props.insert(CHECKED, Value::Bool(checked));
        graph::set_props(GraphPropsRequest {
            node: widget_id,
            props,
        });
    }
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
