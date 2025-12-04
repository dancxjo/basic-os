#![cfg_attr(not(feature = "std"), no_std)]

extern crate alloc;

use alloc::format;
use alloc::string::String;
use alloc::vec;
use userland::flex::{AlignItems, FlexDirection, JustifyContent};
use userland::prelude::*;
use userland::{canon, graph};
use uuid::Uuid;

const OPTIONS_SYM: userland::Symbol = canon::canon(b'O', b'P', b'T');

pub struct PrefsDemoApp {
    _main_window: WindowHandle,
    _dialog_window: WindowHandle,
    _buttons_window: WindowHandle,
    transition_value: Uuid,
    footer_widget: Uuid,
}

impl App for PrefsDemoApp {
    fn init(ctx: &mut AppContext<'_>) -> Self {
        let widget_host_bundle = Uuid::new_v5(&Uuid::NAMESPACE_OID, b"widget_host");

        let transition_value = create_transition_value_node(widget_host_bundle);
        ctx.watch_graph(ThingFilter {
            kind: Some(canon::WIDGET),
            id: Some(transition_value),
        });

        let main_window = create_main_window(ctx, widget_host_bundle);
        let (dialog_window, footer_widget) =
            create_dialog_window(ctx, widget_host_bundle, transition_value);
        let buttons_window = create_buttons_window(ctx, widget_host_bundle);

        PrefsDemoApp {
            _main_window: main_window,
            _dialog_window: dialog_window,
            _buttons_window: buttons_window,
            transition_value,
            footer_widget,
        }
    }

    fn tick(&mut self, _ctx: &mut AppContext<'_>, _tick: u64) {}

    fn on_event(&mut self, _ctx: &mut AppContext<'_>, ev: AppEvent) {
        if let AppEvent::Thing { thing, .. } = ev {
            if thing.id == self.transition_value {
                let value_text = thing
                    .fields
                    .get(&canon::ITEM_VALUE)
                    .and_then(extract_text)
                    .or_else(|| thing.fields.get(&canon::TEXT).and_then(extract_text))
                    .unwrap_or_else(|| String::from("None"));

                let mut fields = graph::map();
                fields.insert(
                    canon::TEXT,
                    Value::Text(format!("Graph emits: {value_text}")),
                );
                graph::fiat(Some(self.footer_widget), canon::WIDGET, fields);
            }
        }
    }
}

fn create_main_window(ctx: &mut AppContext<'_>, widget_host_bundle: Uuid) -> WindowHandle {
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
        mode_index: Some(2), // F3
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

    create_widget(
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
            (canon::cc('F', 'G'), Value::I64(0)),
        ],
        widget_host_bundle,
    );

    create_widget(
        "f3_body",
        "plain_text",
        &window,
        Some(root),
        &[
            (
                canon::TEXT,
                Value::Text(String::from(
                    "The windows in front demonstrate their own layouts, \
including a dropdown control.",
                )),
            ),
            (canon::WIDTH, Value::U64(window_width)),
            (canon::HEIGHT, Value::U64(320)),
            (canon::cc('F', 'G'), Value::I64(0)),
        ],
        widget_host_bundle,
    );

    window
}

fn create_transition_value_node(widget_host_bundle: Uuid) -> Uuid {
    let node_id = Uuid::new_v5(&Uuid::NAMESPACE_OID, b"prefs_transition_value");
    let mut fields = graph::map();
    fields.insert(canon::KIND, Value::Symbol(canon::WIDGET));
    fields.insert(canon::ROLE, Value::Text(String::from("preference.value")));
    fields.insert(
        canon::LABEL,
        Value::Text(String::from("Animation transition preference")),
    );
    fields.insert(canon::SELECTED_INDEX, Value::I64(0));
    fields.insert(canon::ITEM_VALUE, Value::Text(String::from("None")));
    fields.insert(canon::TEXT, Value::Text(String::from("None")));
    fields.insert(canon::VISIBLE, Value::Bool(true));
    graph::fiat(Some(node_id), canon::WIDGET, fields);
    graph::grant_capability(widget_host_bundle, node_id, "CAN_READ");
    graph::grant_capability(widget_host_bundle, node_id, "CAN_WRITE");
    node_id
}

fn create_dialog_window(
    ctx: &mut AppContext<'_>,
    widget_host_bundle: Uuid,
    transition_value_node: Uuid,
) -> (WindowHandle, Uuid) {
    let window_fields = userland::graph::Window {
        id: Uuid::nil(),
        title: String::from("Preferences Sheet"),
        x: 360,
        y: 220,
        width: 440,
        height: 240,
        z: 0,
        visible: true,
        target: None,
        active: false,
        is_root: false,
        mode_index: Some(2), // F3
        window_rect: None,
        gap: Some(12),
        flex_direction: Some(FlexDirection::Column),
        justify_content: Some(JustifyContent::Start),
        align_items: Some(AlignItems::Stretch),
    };
    let window = ctx.create_window_with(window_fields);

    let dialog_root = create_container(
        "prefs_root",
        &window,
        None,
        &[
            (canon::ROLE, Value::Text("window_root".into())),
            (canon::cc('F', 'D'), FlexDirection::Column.to_value()),
            (canon::cc('J', 'C'), JustifyContent::SpaceBetween.to_value()),
            (canon::GAP, Value::I64(10)),
            (canon::cc('F', 'G'), Value::I64(1)),
            (canon::cc('F', 'S'), Value::I64(1)),
        ],
    );

    let content = create_container(
        "prefs_content",
        &window,
        Some(dialog_root),
        &[
            (canon::ROLE, Value::Text("window_root".into())),
            (canon::cc('F', 'D'), FlexDirection::Column.to_value()),
            (canon::GAP, Value::I64(10)),
            (canon::cc('F', 'G'), Value::I64(0)),
            (canon::cc('F', 'S'), Value::I64(1)),
        ],
    );

    let header = create_container(
        "prefs_header",
        &window,
        Some(content),
        &[
            (canon::ROLE, Value::Text("window_root".into())),
            (canon::cc('F', 'D'), FlexDirection::Row.to_value()),
            (canon::GAP, Value::I64(8)),
            (canon::cc('F', 'G'), Value::I64(0)),
        ],
    );
    create_widget(
        "prefs_title",
        "plain_text",
        &window,
        Some(header),
        &[
            (canon::TEXT, Value::Text(String::from("Preferences"))),
            (canon::WIDTH, Value::U64(260)),
            (canon::HEIGHT, Value::U64(24)),
            (canon::cc('F', 'G'), Value::I64(1)),
        ],
        widget_host_bundle,
    );
    create_widget(
        "prefs_close_hint",
        "plain_text",
        &window,
        Some(header),
        &[
            (canon::TEXT, Value::Text(String::from("X"))),
            (canon::WIDTH, Value::U64(24)),
            (canon::HEIGHT, Value::U64(24)),
        ],
        widget_host_bundle,
    );

    let font_row = create_container(
        "prefs_font_row",
        &window,
        Some(content),
        &[
            (canon::ROLE, Value::Text("window_root".into())),
            (canon::cc('F', 'D'), FlexDirection::Row.to_value()),
            (canon::GAP, Value::I64(10)),
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
    let transition_dropdown_id = Uuid::new_v5(&Uuid::NAMESPACE_OID, b"prefs_transition_dropdown");
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
            (canon::BINDS, Value::Uuid(transition_value_node)),
            (
                OPTIONS_SYM,
                Value::List(vec![
                    Value::Text(String::from("None")),
                    Value::Text(String::from("Slide")),
                    Value::Text(String::from("Fade")),
                    Value::Text(String::from("Zoom")),
                ]),
            ),
            (canon::cc('F', 'G'), Value::I64(1)),
        ],
        widget_host_bundle,
    );

    let footer_hint = create_widget(
        "prefs_footer_hint",
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

fn create_buttons_window(ctx: &mut AppContext<'_>, widget_host_bundle: Uuid) -> WindowHandle {
    let window_fields = userland::graph::Window {
        id: Uuid::nil(),
        title: String::from("Button Gallery"),
        x: 820,
        y: 220,
        width: 300,
        height: 400,
        z: 0,
        visible: true,
        target: None,
        active: false,
        is_root: false,
        mode_index: Some(2), // F3
        window_rect: None,
        gap: Some(12),
        flex_direction: Some(FlexDirection::Column),
        justify_content: Some(JustifyContent::Start),
        align_items: Some(AlignItems::Stretch),
    };
    let window = ctx.create_window_with(window_fields);

    let root = create_container(
        "buttons_root",
        &window,
        None,
        &[
            (canon::ROLE, Value::Text("window_root".into())),
            (canon::cc('F', 'D'), FlexDirection::Column.to_value()),
            (canon::cc('J', 'C'), JustifyContent::Start.to_value()),
            (canon::GAP, Value::I64(16)),
            (canon::cc('F', 'G'), Value::I64(1)),
            (canon::cc('F', 'S'), Value::I64(1)),
            (canon::cc('A', 'I'), AlignItems::Center.to_value()),
            (canon::cc('P', 'T'), Value::I64(16)), // Padding Top
            (canon::cc('P', 'B'), Value::I64(16)), // Padding Bottom
            (canon::cc('P', 'L'), Value::I64(16)), // Padding Left
            (canon::cc('P', 'R'), Value::I64(16)), // Padding Right
        ],
    );

    // 1. Standard Button
    create_widget(
        "btn_standard",
        "button",
        &window,
        Some(root),
        &[
            (canon::TEXT, Value::Text("Standard Button".into())),
            (canon::WIDTH, Value::U64(200)),
            (canon::HEIGHT, Value::U64(32)),
        ],
        widget_host_bundle,
    );

    // 2. Icon Button
    create_widget(
        "btn_icon",
        "button",
        &window,
        Some(root),
        &[
            (canon::TEXT, Value::Text("Settings".into())),
            (canon::ICON_NAME, Value::Text("settings".into())),
            (canon::WIDTH, Value::U64(200)),
            (canon::HEIGHT, Value::U64(32)),
        ],
        widget_host_bundle,
    );

    // 3. Icon Only Button
    create_widget(
        "btn_icon_only",
        "button",
        &window,
        Some(root),
        &[
            (canon::ICON_NAME, Value::Text("home".into())),
            (canon::canon(b'S', b'H', b'L'), Value::Bool(false)), // Hide label
            (canon::WIDTH, Value::U64(48)),
            (canon::HEIGHT, Value::U64(48)),
        ],
        widget_host_bundle,
    );

    // 4. Long Text Button
    create_widget(
        "btn_long",
        "button",
        &window,
        Some(root),
        &[
            (canon::TEXT, Value::Text("A Very Long Button Label".into())),
            (canon::WIDTH, Value::U64(240)),
            (canon::HEIGHT, Value::U64(32)),
        ],
        widget_host_bundle,
    );

    window
}
