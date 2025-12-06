use alloc::collections::BTreeMap;
use alloc::format;
use alloc::string::ToString;
use userland::flex::{AlignItems, FlexDirection, JustifyContent};
use userland::graph::GraphPropsRequest;
use userland::layout_instantiator::LayoutInstantiator;
use userland::prelude::*;
use userland::{canon, graph, AppEvent, ThingFilter, Value};
use uuid::Uuid;

use crate::template_builder::{RegionBuilder, TemplateBuilder};

const TOOLBAR_REGION: &str = "demo_toolbar";
const LIST_REGION: &str = "demo_primary_list";
const INSPECTOR_REGION: &str = "demo_inspector";

pub struct DemoApp {
    window: WindowHandle,
    selection_node: Uuid,
    list_widget_id: Uuid,
    inspector_widget_id: Uuid,
    current_index: usize,
    entries: &'static [DashboardEntry],
}

struct DashboardEntry {
    label: &'static str,
    target: &'static str,
    icon: &'static str,
}

static DASHBOARD_ENTRIES: &[DashboardEntry] = &[
    DashboardEntry {
        label: "Text Editor",
        target: "text_editor",
        icon: "menu",
    },
    DashboardEntry {
        label: "Graph Viewer",
        target: "graph_viewer",
        icon: "home",
    },
    DashboardEntry {
        label: "Thing Viewer",
        target: "thing_viewer",
        icon: "settings",
    },
    DashboardEntry {
        label: "Self Edit",
        target: "self_editing_demo",
        icon: "arrow-back",
    },
];

impl App for DemoApp {
    fn init(ctx: &mut AppContext<'_>) -> Self {
        let widget_host_bundle = Uuid::new_v5(&Uuid::NAMESPACE_OID, b"widget_host");
        let entries = DASHBOARD_ENTRIES;

        let window_fields = userland::graph::Window {
            id: Uuid::nil(),
            title: "Layout Showcase".to_string(),
            x: 32,
            y: 32,
            width: 1280,
            height: 800,
            z: 0,
            visible: true,
            target: None,
            active: false,
            is_root: true,
            is_place_root: false,
            place_id: None,
            mode_index: None,
            window_rect: None,
            gap: Some(12),
            flex_direction: Some(FlexDirection::Column),
            justify_content: Some(JustifyContent::Start),
            align_items: Some(AlignItems::Stretch),
            tile_mode: None,
        };
        let window = ctx.create_window_with(window_fields);

        let selection_node = userland::simple_uuid(b"demo_layout.selection");
        let mut selection_fields = graph::map();
        selection_fields.insert(canon::SELECTED_INDEX, Value::I64(0));
        graph::fiat(Some(selection_node), canon::WIDGET, selection_fields);

        let template = build_template(entries);

        let mut bindings = BTreeMap::new();
        bindings.insert("selection".to_string(), selection_node);

        LayoutInstantiator::instantiate(
            window.window_id(),
            template.id(),
            &bindings,
            widget_host_bundle,
        );

        let list_widget_id = widget_id_for(window.window_id(), LIST_REGION);
        let inspector_widget_id = widget_id_for(window.window_id(), INSPECTOR_REGION);

        populate_launcher_items(list_widget_id, widget_host_bundle);

        let mut app = DemoApp {
            window,
            selection_node,
            list_widget_id,
            inspector_widget_id,
            current_index: 0,
            entries,
        };

        app.update_selection(0);

        ctx.watch_graph(ThingFilter {
            kind: Some(canon::WIDGET),
            id: Some(selection_node),
        });

        app
    }

    fn on_event(&mut self, _ctx: &mut AppContext<'_>, ev: AppEvent) {
        if let AppEvent::Thing { thing, .. } = ev {
            if thing.id == self.selection_node {
                if let Some(Value::I64(idx)) = thing.fields.get(&canon::SELECTED_INDEX) {
                    let idx = (*idx).max(0) as usize;
                    if idx < self.entries.len() && idx != self.current_index {
                        self.update_selection(idx);
                    }
                }
            }
        }
    }

    fn tick(&mut self, _ctx: &mut AppContext<'_>, _tick: u64) {}
}

impl DemoApp {
    fn update_selection(&mut self, idx: usize) {
        let entry = &self.entries[idx];
        self.current_index = idx;

        let mut window_props = graph::map();
        window_props.insert(
            canon::TITLE,
            Value::Text(format!("Layout Showcase — {}", entry.label)),
        );
        graph::set_props(GraphPropsRequest {
            node: self.window.window_id(),
            props: window_props,
        });

        let mut inspector_props = graph::map();
        inspector_props.insert(canon::LABEL, Value::Text(entry.label.into()));
        inspector_props.insert(
            canon::TITLE,
            Value::Text(format!("Inspect {}", entry.label)),
        );
        inspector_props.insert(canon::TEXT, Value::Text(entry.target.into()));
        inspector_props.insert(canon::KIND, Value::Text(entry.target.into()));
        graph::set_props(GraphPropsRequest {
            node: self.inspector_widget_id,
            props: inspector_props,
        });
    }
}

fn build_template(entries: &[DashboardEntry]) -> TemplateBuilder {
    let template = TemplateBuilder::new("demo_flex_layout", "container.vertical", Some(1));
    let root = build_root_region(entries);
    template.add_region(root);
    template
}

fn build_root_region(entries: &[DashboardEntry]) -> RegionBuilder {
    let mut root = RegionBuilder::new("demo_root")
        .role("container.vertical")
        .flex_dir(FlexDirection::Column)
        .justify_content(JustifyContent::Start)
        .align_items(AlignItems::Stretch)
        .gap(12)
        .grow(1)
        .shrink(1);

    root = root.child(build_toolbar_region(entries));
    root = root.child(build_content_region());
    root
}

fn build_toolbar_region(entries: &[DashboardEntry]) -> RegionBuilder {
    let mut toolbar = RegionBuilder::new(TOOLBAR_REGION)
        .role("container.toolbar")
        .kind("toolbar")
        .flex_dir(FlexDirection::Row)
        .justify_content(JustifyContent::Start)
        .align_items(AlignItems::Center)
        .gap(6)
        .height(40);

    for entry in entries {
        let name = format!("toolbar_btn_{}", entry.target);
        toolbar = toolbar.child(
            RegionBuilder::new(&name)
                .role("control.toolbar_button")
                .kind("toolbar_button")
                .text(entry.label)
                .icon(entry.icon)
                .focusable(true),
        );
    }

    toolbar
}

fn build_content_region() -> RegionBuilder {
    let mut content = RegionBuilder::new("demo_content_row")
        .role("container.vertical")
        .flex_dir(FlexDirection::Row)
        .justify_content(JustifyContent::Start)
        .align_items(AlignItems::Stretch)
        .gap(12)
        .grow(1)
        .shrink(1);

    content = content.child(
        RegionBuilder::new(LIST_REGION)
            .role("primary_list")
            .kind("listbox_default")
            .width(320)
            .grow(1)
            .shrink(0)
            .binds_to("selection")
            .focusable(true),
    );

    content = content.child(
        RegionBuilder::new(INSPECTOR_REGION)
            .role("thing_tile")
            .kind("thing_tile")
            .grow(2)
            .shrink(1),
    );

    content
}

fn widget_id_for(window_id: Uuid, region_name: &str) -> Uuid {
    let region_id = Uuid::new_v5(&Uuid::NAMESPACE_OID, region_name.as_bytes());
    let seed = format!("{}-{}-widget", window_id, region_id);
    userland::simple_uuid(seed.as_bytes())
}

fn populate_launcher_items(list_id: Uuid, widget_host_bundle: Uuid) {
    for entry in DASHBOARD_ENTRIES {
        let item_id = Uuid::new_v5(&Uuid::NAMESPACE_OID, entry.label.as_bytes());
        let mut fields = graph::map();
        fields.insert(canon::KIND, Value::Symbol(canon::ITEM));
        fields.insert(canon::PARENT, Value::Uuid(list_id));
        fields.insert(canon::ITEM_LABEL, Value::Text(entry.label.into()));
        fields.insert(canon::ITEM_VALUE, Value::Text(entry.target.into()));
        fields.insert(canon::ICON_NAME, Value::Text(entry.icon.into()));
        fields.insert(canon::VISIBLE, Value::Bool(true));
        graph::fiat(Some(item_id), canon::ITEM, fields);

        graph::grant_capability(widget_host_bundle, item_id, "CAN_READ");
    }
}
