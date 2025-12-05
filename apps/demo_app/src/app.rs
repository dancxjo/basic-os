use alloc::collections::{BTreeMap, VecDeque};
use alloc::format;
use alloc::string::String;
use alloc::string::ToString;
use alloc::vec;
use userland::flex::{AlignItems, FlexDirection, JustifyContent};
use userland::graph::{GraphPropsGetRequest, GraphPropsRequest};
use userland::prelude::*;
use userland::questions::{
    attach_question_to_form, ensure_action_interaction, ensure_form, ensure_question_interaction,
    ensure_question_with_answer,
};
use userland::{
    canon, graph, AnswerValue, AppEvent, InteractionBinding, QuestionBinding, Symbol, ThingFilter,
};
use uuid::Uuid;
use widget_checkbox::CHECKED;

const TRANSITION_OPTIONS: &[&str] = &["None", "Slide", "Fade", "Zoom"];

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
                (canon::cc('F', 'D'), FlexDirection::Row.to_value()),
                (canon::GAP, Value::I64((sizes.gap / 2).into())),
                (canon::HEIGHT, Value::U64(40)),
                (canon::cc('F', 'G'), Value::I64(0)),
                (canon::cc('F', 'S'), Value::I64(0)),
            ],
        );
        for (i, entry) in DASHBOARD_ENTRIES.iter().enumerate() {
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
                    (canon::ROLE, Value::Text("control.toolbar_button".into())),
                    (canon::BINDS, Value::Uuid(selection_node)),
                    (canon::canon(b'I', b'D', b'X'), Value::I64(i as i64)),
                    (canon::FOCUSABLE, Value::Bool(true)),
                ],
                widget_host_bundle,
            );
        }

        // Body container (row: rail | main | sidebar)
        let body_id = create_container(
            "body_container",
            &window,
            None,
            &[
                (canon::ROLE, Value::Text("window_root".into())),
                (canon::cc('F', 'D'), FlexDirection::Row.to_value()),
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
                (canon::FOCUSABLE, Value::Bool(true)),
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
                (
                    canon::cc('I', 'D'),
                    Value::Bytes(build_solid_image(400, 400, 200, 200, 200)),
                ),
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
                (canon::cc('F', 'D'), FlexDirection::Column.to_value()),
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

        // Controls panel (checkbox/radio/image)
        let controls_panel = create_container(
            "controls_panel",
            &window,
            Some(sidebar_id),
            &[
                (canon::ROLE, Value::Text("window_root".into())),
                (canon::cc('F', 'D'), FlexDirection::Column.to_value()),
                (canon::GAP, Value::I64(6)),
                (canon::cc('F', 'G'), Value::I64(0)),
                (canon::cc('F', 'S'), Value::I64(0)),
            ],
        );

        create_widget(
            "control_checkbox",
            "checkbox",
            &window,
            Some(controls_panel),
            &[
                (canon::TEXT, Value::Text("Enable feature".into())),
                (canon::WIDTH, Value::U64(160)),
                (canon::HEIGHT, Value::U64(28)),
                (canon::FOCUSABLE, Value::Bool(true)),
            ],
            widget_host_bundle,
        );

        create_widget(
            "control_radio_a",
            "radio_button",
            &window,
            Some(controls_panel),
            &[
                (canon::TEXT, Value::Text("Mode A".into())),
                (canon::WIDTH, Value::U64(140)),
                (canon::HEIGHT, Value::U64(28)),
                (canon::FOCUSABLE, Value::Bool(true)),
            ],
            widget_host_bundle,
        );

        create_widget(
            "control_radio_b",
            "radio_button",
            &window,
            Some(controls_panel),
            &[
                (canon::TEXT, Value::Text("Mode B".into())),
                (canon::WIDTH, Value::U64(140)),
                (canon::HEIGHT, Value::U64(28)),
                (canon::FOCUSABLE, Value::Bool(true)),
            ],
            widget_host_bundle,
        );

        create_widget(
            "control_image",
            "image",
            &window,
            Some(controls_panel),
            &[
                (canon::WIDTH, Value::U64(120)),
                (canon::HEIGHT, Value::U64(80)),
                (
                    canon::cc('I', 'D'),
                    Value::Bytes(build_solid_image(120, 80, 0x70, 0x90, 0xFF)),
                ),
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

        let prefs = PrefsShowcase::new(ctx, widget_host_bundle);
        let questions = QuestionShowcase::new(ctx, widget_host_bundle);

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

const OPTIONS_SYM: Symbol = canon::canon(b'O', b'P', b'T');
const QUESTION_ROW_HEIGHT: u64 = 48;

struct PrefsShowcase {
    main_window: WindowHandle,
    dialog_window: WindowHandle,
    buttons_window: WindowHandle,
    transition_value: Uuid,
    main_footer_widget: Uuid,
    dialog_footer_widget: Uuid,
}

impl PrefsShowcase {
    fn new(ctx: &mut AppContext<'_>, widget_host_bundle: Uuid) -> Self {
        let transition_value = create_transition_value_node(widget_host_bundle);
        ctx.watch_graph(ThingFilter {
            kind: Some(canon::WIDGET),
            id: Some(transition_value),
        });

        let (main_window, main_footer_widget) =
            Self::create_main_window(ctx, widget_host_bundle, transition_value);
        let (dialog_window, dialog_footer_widget) =
            Self::create_dialog_window(ctx, widget_host_bundle, transition_value);
        let buttons_window = Self::create_buttons_window(ctx, widget_host_bundle);

        PrefsShowcase {
            main_window,
            dialog_window,
            buttons_window,
            transition_value,
            main_footer_widget,
            dialog_footer_widget,
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
        graph::fiat(Some(self.main_footer_widget), canon::WIDGET, fields.clone());
        graph::fiat(Some(self.dialog_footer_widget), canon::WIDGET, fields);
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
            mode_index: Some(2),
            window_rect: None,
            gap: Some(16),
            flex_direction: Some(FlexDirection::Column),
            justify_content: Some(JustifyContent::Start),
            align_items: Some(AlignItems::Stretch),
        };
        let window = ctx.create_window_with(window_fields);

        let root = create_container(
            "f3_root",
            &window,
            None,
            &[
                (canon::ROLE, Value::Text("window_root".into())),
                (canon::cc('F', 'D'), FlexDirection::Column.to_value()),
                (canon::cc('J', 'C'), JustifyContent::SpaceBetween.to_value()),
                (canon::GAP, Value::I64(12)),
                (canon::cc('F', 'G'), Value::I64(1)),
                (canon::cc('F', 'S'), Value::I64(1)),
                (canon::cc('A', 'I'), AlignItems::Stretch.to_value()),
            ],
        );

        let intro = create_widget(
            "f3_intro",
            "plain_text",
            &window,
            Some(root),
            &[
                (
                    canon::TEXT,
                    Value::Text(String::from(
                        "This root window sits on Mode F3. It uses a plain text widget so we can \
show a simple, readable message directly in the client area.",
                    )),
                ),
                (canon::WIDTH, Value::U64(window_width)),
                (canon::HEIGHT, Value::U64(120)),
                (canon::cc('F', 'S'), Value::I64(0)),
                (canon::cc('F', 'G'), Value::I64(0)),
            ],
            widget_host_bundle,
        );
        graph::grant_capability(widget_host_bundle, intro, "CAN_FOCUS");

        let content = create_container(
            "f3_content",
            &window,
            Some(root),
            &[
                (canon::ROLE, Value::Text("content".into())),
                (canon::cc('F', 'D'), FlexDirection::Column.to_value()),
                (canon::GAP, Value::I64(20)),
                (canon::cc('F', 'G'), Value::I64(1)),
                (canon::cc('F', 'S'), Value::I64(1)),
            ],
        );

        let font_row = create_container(
            "prefs_font_row",
            &window,
            Some(content),
            &[
                (canon::cc('F', 'D'), FlexDirection::Row.to_value()),
                (canon::GAP, Value::I64(8)),
                (canon::cc('F', 'G'), Value::I64(0)),
            ],
        );
        create_widget(
            "prefs_font_label",
            "plain_text",
            &window,
            Some(font_row),
            &[
                (canon::TEXT, Value::Text(String::from("Font:"))),
                (canon::WIDTH, Value::U64(80)),
                (canon::HEIGHT, Value::U64(24)),
            ],
            widget_host_bundle,
        );
        create_widget(
            "prefs_font_value",
            "plain_text",
            &window,
            Some(font_row),
            &[
                (canon::TEXT, Value::Text(String::from("Monospace 12"))),
                (canon::WIDTH, Value::U64(180)),
                (canon::HEIGHT, Value::U64(28)),
                (canon::cc('F', 'G'), Value::I64(1)),
            ],
            widget_host_bundle,
        );

        let transition_row = create_container(
            "prefs_transition_row",
            &window,
            Some(content),
            &[
                (canon::ROLE, Value::Text("window_root".into())),
                (canon::cc('F', 'D'), FlexDirection::Row.to_value()),
                (canon::GAP, Value::I64(10)),
                (canon::cc('F', 'G'), Value::I64(0)),
            ],
        );
        let transition_dropdown_id =
            Uuid::new_v5(&Uuid::NAMESPACE_OID, b"prefs_transition_dropdown");
        create_widget(
            "prefs_transition_label",
            "plain_text",
            &window,
            Some(transition_row),
            &[
                (canon::TEXT, Value::Text(String::from("Transition:"))),
                (canon::WIDTH, Value::U64(120)),
                (canon::HEIGHT, Value::U64(24)),
                (canon::ROLE, Value::Text(String::from("label"))),
                (canon::LABEL_FOR, Value::Uuid(transition_dropdown_id)),
            ],
            widget_host_bundle,
        );
        create_widget(
            "prefs_transition_dropdown",
            "select",
            &window,
            Some(transition_row),
            &[
                (canon::TEXT, Value::Text(String::from("None"))),
                (canon::WIDTH, Value::U64(200)),
                (canon::HEIGHT, Value::U64(32)),
                (canon::FOCUSABLE, Value::Bool(true)),
                (canon::SELECTED_INDEX, Value::I64(0)),
                (canon::ROLE, Value::Text(String::from("select"))),
                (canon::BINDS, Value::Uuid(transition_value)),
                (
                    OPTIONS_SYM,
                    Value::List(
                        TRANSITION_OPTIONS
                            .iter()
                            .map(|s| Value::Text(String::from(*s)))
                            .collect(),
                    ),
                ),
                (canon::cc('F', 'G'), Value::I64(1)),
            ],
            widget_host_bundle,
        );

        let footer_hint = create_widget(
            "prefs_footer_hint",
            "plain_text",
            &window,
            Some(content),
            &[
                (canon::TEXT, Value::Text(String::from("Graph emits: None"))),
                (canon::WIDTH, Value::U64(300)),
                (canon::HEIGHT, Value::U64(28)),
            ],
            widget_host_bundle,
        );
        graph::that(window.window_id(), "contains", footer_hint, 0);

        (window, footer_hint)
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
            mode_index: None,
            window_rect: None,
            gap: Some(12),
            flex_direction: Some(FlexDirection::Column),
            justify_content: Some(JustifyContent::Start),
            align_items: Some(AlignItems::Stretch),
        };
        let window = ctx.create_window_with(window_fields);

        let dialog_root = create_container(
            "prefs_dialog_root",
            &window,
            None,
            &[
                (canon::ROLE, Value::Text("window_root".into())),
                (canon::cc('F', 'D'), FlexDirection::Column.to_value()),
                (canon::cc('J', 'C'), JustifyContent::Start.to_value()),
                (canon::GAP, Value::I64(10)),
                (canon::cc('F', 'G'), Value::I64(1)),
                (canon::cc('F', 'S'), Value::I64(1)),
            ],
        );

        let buttons_row = create_container(
            "prefs_buttons",
            &window,
            Some(dialog_root),
            &[
                (canon::cc('F', 'D'), FlexDirection::Row.to_value()),
                (canon::GAP, Value::I64(16)),
                (canon::cc('F', 'G'), Value::I64(0)),
            ],
        );
        let confirm_button = create_widget(
            "prefs_confirm",
            "button",
            &window,
            Some(buttons_row),
            &[
                (canon::TEXT, Value::Text(String::from("Confirm"))),
                (canon::WIDTH, Value::U64(120)),
                (canon::HEIGHT, Value::U64(32)),
                (canon::cc('F', 'G'), Value::I64(1)),
                (canon::FOCUSABLE, Value::Bool(true)),
            ],
            widget_host_bundle,
        );
        let cancel_button = create_widget(
            "prefs_cancel",
            "button",
            &window,
            Some(buttons_row),
            &[
                (canon::TEXT, Value::Text(String::from("Cancel"))),
                (canon::WIDTH, Value::U64(120)),
                (canon::HEIGHT, Value::U64(32)),
                (canon::cc('F', 'G'), Value::I64(1)),
                (canon::FOCUSABLE, Value::Bool(true)),
            ],
            widget_host_bundle,
        );
        ensure_action_interaction(
            transition_value_node,
            Some(confirm_button),
            Some("Apply transition"),
            Some("Act on the current transition choice"),
        );
        ensure_action_interaction(
            transition_value_node,
            Some(cancel_button),
            Some("Cancel transition"),
            Some("Dismiss transition changes"),
        );

        let transition_row = create_container(
            "prefs_dialog_transition",
            &window,
            Some(dialog_root),
            &[
                (canon::cc('F', 'D'), FlexDirection::Row.to_value()),
                (canon::GAP, Value::I64(12)),
            ],
        );
        create_widget(
            "prefs_dialog_transition_label",
            "plain_text",
            &window,
            Some(transition_row),
            &[
                (canon::TEXT, Value::Text(String::from("Transition:"))),
                (canon::WIDTH, Value::U64(120)),
                (canon::HEIGHT, Value::U64(28)),
                (canon::ROLE, Value::Text(String::from("label"))),
            ],
            widget_host_bundle,
        );
        create_widget(
            "prefs_dialog_transition_select",
            "select",
            &window,
            Some(transition_row),
            &[
                (canon::TEXT, Value::Text(String::from("None"))),
                (canon::WIDTH, Value::U64(220)),
                (canon::HEIGHT, Value::U64(32)),
                (canon::FOCUSABLE, Value::Bool(true)),
                (canon::SELECTED_INDEX, Value::I64(0)),
                (canon::ROLE, Value::Text(String::from("select"))),
                (canon::BINDS, Value::Uuid(transition_value_node)),
                (
                    OPTIONS_SYM,
                    Value::List(
                        TRANSITION_OPTIONS
                            .iter()
                            .map(|s| Value::Text(String::from(*s)))
                            .collect(),
                    ),
                ),
            ],
            widget_host_bundle,
        );

        let footer_hint = create_widget(
            "prefs_dialog_footer_hint",
            "plain_text",
            &window,
            Some(dialog_root),
            &[
                (canon::TEXT, Value::Text(String::from("Graph emits: None"))),
                (canon::WIDTH, Value::U64(300)),
                (canon::HEIGHT, Value::U64(28)),
            ],
            widget_host_bundle,
        );
        (window, footer_hint)
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
            mode_index: None,
            window_rect: None,
            gap: Some(12),
            flex_direction: Some(FlexDirection::Column),
            justify_content: Some(JustifyContent::Start),
            align_items: Some(AlignItems::Stretch),
        };
        let window = ctx.create_window_with(window_fields);

        let button_root = create_container(
            "prefs_button_root",
            &window,
            None,
            &[
                (canon::cc('F', 'D'), FlexDirection::Column.to_value()),
                (canon::GAP, Value::I64(10)),
            ],
        );
        create_widget(
            "prefs_button_primary",
            "button",
            &window,
            Some(button_root),
            &[
                (canon::TEXT, Value::Text(String::from("Primary"))),
                (canon::WIDTH, Value::U64(160)),
                (canon::HEIGHT, Value::U64(36)),
                (canon::FOCUSABLE, Value::Bool(true)),
            ],
            widget_host_bundle,
        );
        create_widget(
            "prefs_button_secondary",
            "button",
            &window,
            Some(button_root),
            &[
                (canon::TEXT, Value::Text(String::from("Secondary"))),
                (canon::WIDTH, Value::U64(160)),
                (canon::HEIGHT, Value::U64(36)),
                (canon::FOCUSABLE, Value::Bool(true)),
            ],
            widget_host_bundle,
        );

        let toggle_row = create_container(
            "prefs_button_toggle_row",
            &window,
            Some(button_root),
            &[(canon::cc('F', 'D'), FlexDirection::Row.to_value())],
        );
        create_widget(
            "prefs_toggle_label",
            "plain_text",
            &window,
            Some(toggle_row),
            &[
                (canon::TEXT, Value::Text(String::from("Notifications"))),
                (canon::WIDTH, Value::U64(180)),
                (canon::HEIGHT, Value::U64(24)),
            ],
            widget_host_bundle,
        );
        create_widget(
            "prefs_toggle",
            "checkbox",
            &window,
            Some(toggle_row),
            &[
                (canon::WIDTH, Value::U64(120)),
                (canon::HEIGHT, Value::U64(24)),
                (canon::FOCUSABLE, Value::Bool(true)),
                (CHECKED, Value::Bool(true)),
            ],
            widget_host_bundle,
        );

        window
    }
}

struct BoundQuestion {
    widget_id: Uuid,
    binding: QuestionBinding,
    interaction: InteractionBinding,
}

struct QuestionShowcase {
    window: WindowHandle,
    questions: Vec<BoundQuestion>,
}

impl QuestionShowcase {
    fn new(ctx: &mut AppContext<'_>, widget_host_bundle: Uuid) -> Self {
        let (window, root) = Self::create_window(ctx);
        let form_id = ensure_form(None, "Semantic question demo");

        let mut questions = Vec::new();
        let question_one_label = "Enable Neo4j backend?";
        let question_one_desc = "Toggles the graph persistence layer";
        let (q1, _a1, b1) = ensure_question_with_answer(
            None,
            None,
            question_one_label,
            Some(question_one_desc),
            AnswerValue::Bool(false),
        );
        attach_question_to_form(form_id, q1);
        let neo_widget =
            Self::create_yes_no_widget("neo4j_toggle", &window, root, &b1, widget_host_bundle);
        questions.push(BoundQuestion {
            widget_id: neo_widget,
            binding: b1,
            interaction: ensure_question_interaction(
                &b1,
                Some(neo_widget),
                Some(question_one_label),
                Some(question_one_desc),
            ),
        });

        let question_two_label = "Allow telemetry?";
        let question_two_desc = "Share anonymous usage to improve stability";
        let (q2, _a2, b2) = ensure_question_with_answer(
            None,
            None,
            question_two_label,
            Some(question_two_desc),
            AnswerValue::Bool(true),
        );
        attach_question_to_form(form_id, q2);
        let telemetry_widget =
            Self::create_yes_no_widget("telemetry_toggle", &window, root, &b2, widget_host_bundle);
        questions.push(BoundQuestion {
            widget_id: telemetry_widget,
            binding: b2,
            interaction: ensure_question_interaction(
                &b2,
                Some(telemetry_widget),
                Some(question_two_label),
                Some(question_two_desc),
            ),
        });

        for bound in &questions {
            ctx.watch_graph(ThingFilter {
                kind: Some(canon::WIDGET),
                id: Some(bound.widget_id),
            });
            ctx.watch_graph(ThingFilter {
                kind: Some(canon::ANSWER),
                id: Some(bound.binding.answer),
            });
            ctx.watch_graph(ThingFilter {
                kind: Some(canon::INTERACTION),
                id: Some(bound.interaction.interaction),
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

            if thing.id == bound.interaction.interaction {
                if let Some(label) = thing
                    .fields
                    .get(&canon::LABEL)
                    .and_then(graph::extract_text)
                {
                    Self::push_widget_label(bound.widget_id, &label);
                }
            }
        }
    }

    fn create_window(ctx: &mut AppContext<'_>) -> (WindowHandle, Uuid) {
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
            mode_index: Some(3),
            window_rect: None,
            gap: Some(12),
            flex_direction: Some(FlexDirection::Column),
            justify_content: Some(JustifyContent::Start),
            align_items: Some(AlignItems::Stretch),
        };
        let window = ctx.create_window_with(window_fields);

        let root = create_container(
            "question_root",
            &window,
            None,
            &[
                (canon::ROLE, Value::Text("window_root".into())),
                (canon::cc('F', 'D'), FlexDirection::Column.to_value()),
                (canon::GAP, Value::I64(12)),
                (canon::cc('F', 'G'), Value::I64(1)),
                (canon::cc('F', 'S'), Value::I64(1)),
                (canon::cc('A', 'I'), AlignItems::Stretch.to_value()),
            ],
        );

        Self::create_intro_text(&window, root);
        (window, root)
    }

    fn create_yes_no_widget(
        name: &str,
        window: &WindowHandle,
        parent: Uuid,
        binding: &QuestionBinding,
        widget_host_bundle: Uuid,
    ) -> Uuid {
        let widget_id = Uuid::new_v5(&Uuid::NAMESPACE_OID, name.as_bytes());
        let mut fields = graph::map();
        fields.insert(canon::cc('W', 'K'), Value::Text("checkbox".into()));
        fields.insert(canon::PARENT, Value::Uuid(parent));
        fields.insert(canon::TEXT, Value::Text(Self::load_label(binding)));
        fields.insert(canon::HEIGHT, Value::U64(QUESTION_ROW_HEIGHT));
        fields.insert(canon::WIDTH, Value::U64(320));
        fields.insert(canon::VISIBLE, Value::Bool(true));
        fields.insert(CHECKED, Value::Bool(Self::load_answer(binding)));
        fields.insert(canon::FOCUSABLE, Value::Bool(true));
        graph::fiat(Some(widget_id), canon::WIDGET, fields);
        graph::grant_capability(widget_host_bundle, widget_id, "CAN_READ");
        graph::that(window.window_id(), "contains", widget_id, 0);
        widget_id
    }

    fn create_intro_text(window: &WindowHandle, parent: Uuid) {
        let widget_id = Uuid::new_v5(&Uuid::NAMESPACE_OID, b"question_intro");
        let mut fields = graph::map();
        fields.insert(canon::PARENT, Value::Uuid(parent));
        fields.insert(canon::cc('W', 'K'), Value::Text("plain_text".into()));
        fields.insert(
            canon::TEXT,
            Value::Text(
                "Questions are graph things. Widgets are just one view over those semantic nodes."
                    .into(),
            ),
        );
        fields.insert(canon::HEIGHT, Value::U64(72));
        fields.insert(canon::WIDTH, Value::U64(440));
        fields.insert(canon::cc('F', 'S'), Value::I64(0));
        graph::fiat(Some(widget_id), canon::WIDGET, fields);
        graph::that(window.window_id(), "contains", widget_id, 0);
    }

    fn load_label(binding: &QuestionBinding) -> String {
        let request = GraphPropsGetRequest {
            node: binding.question,
            keys: alloc::vec![canon::LABEL],
        };
        match graph::get_props(request)
            .as_ref()
            .and_then(|props| graph::extract_text(props.get(&canon::LABEL)?))
        {
            Some(label) => label,
            None => "Question".to_string(),
        }
    }

    fn load_answer(binding: &QuestionBinding) -> bool {
        let request = GraphPropsGetRequest {
            node: binding.answer,
            keys: alloc::vec![canon::VALUE_BOOL],
        };
        match graph::get_props(request)
            .as_ref()
            .and_then(|props| props.get(&canon::VALUE_BOOL).and_then(|v| v.as_bool()))
        {
            Some(val) => val,
            None => false,
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

    fn push_widget_label(widget_id: Uuid, label: &str) {
        let mut props = graph::map();
        props.insert(canon::TEXT, Value::Text(label.into()));
        graph::set_props(GraphPropsRequest {
            node: widget_id,
            props,
        });
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
