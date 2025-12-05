//! Helpers for defining widget and cursor metadata in the graph.
//!
//! The utilities in this module describe the UI surface in semantic terms
//! (roles, labels, layout hints) without binding to a particular renderer or
//! widget implementation. Applications can build a UI tree by emitting
//! `WIDGET` and `CURSOR` Things with the appropriate properties, then attach
//! their own rendering logic on top.
use crate::app::AppContext;
use crate::flex::{AlignItems, FlexDirection, JustifyContent};
use crate::{canon, graph, Symbol, Thingable, Value};
use alloc::string::String;
use thing_abi::GraphThing;
use uuid::Uuid;

#[derive(Clone, Debug)]
pub struct Widget {
    pub id: Uuid,
    pub role: String,
    pub kind: Option<String>,
    pub parent: Option<Uuid>,
    pub visible: bool,
    pub enabled: bool,
    pub label: Option<String>,
    pub icon: Option<String>,
    pub action: Option<String>,
    pub description: Option<String>,
    pub focusable: bool,
    pub tab_index: Option<i64>,
    pub bitmap: Option<alloc::vec::Vec<u8>>,
    /// Preferred width (layout hint).
    pub width: Option<u64>,
    /// Preferred height (layout hint).
    pub height: Option<u64>,
    /// Minimum width (overrides default 0).
    pub min_width: Option<u64>,
    /// Minimum height (overrides default 0).
    pub min_height: Option<u64>,
    /// Maximum width.
    pub max_width: Option<u64>,
    /// Maximum height.
    pub max_height: Option<u64>,
    pub x: Option<u64>,
    pub y: Option<u64>,
    pub flex_direction: Option<FlexDirection>,
    pub justify_content: Option<JustifyContent>,
    pub align_items: Option<AlignItems>,
    /// Flex grow factor (default 0.0).
    pub flex_grow: Option<f32>,
    /// Flex shrink factor (default 1.0).
    pub flex_shrink: Option<f32>,
    /// Gap between children (if container).
    pub gap: Option<i32>,
}

impl Widget {
    pub fn update(&mut self, thing: &GraphThing) {
        if let Some(role) = thing
            .fields
            .get(&canon::ROLE)
            .and_then(graph::extract_text)
            .or_else(|| {
                thing
                    .fields
                    .get(&canon::cc('W', 'K'))
                    .and_then(graph::extract_text)
            })
        {
            self.role = role;
        }

        if let Some(kind) = thing
            .fields
            .get(&canon::cc('W', 'K'))
            .and_then(graph::extract_text)
        {
            self.kind = Some(kind);
        }

        if let Some(parent) = thing.fields.get(&canon::PARENT).and_then(|v| v.as_uuid()) {
            self.parent = Some(parent);
        }

        if let Some(visible) = thing.fields.get(&canon::VISIBLE).and_then(|v| v.as_bool()) {
            self.visible = visible;
        }

        if let Some(enabled) = thing.fields.get(&canon::ACTIVE).and_then(|v| v.as_bool()) {
            self.enabled = enabled;
        }

        if let Some(label) = thing
            .fields
            .get(&canon::LABEL)
            .or_else(|| thing.fields.get(&canon::ITEM_LABEL))
            .and_then(graph::extract_text)
        {
            self.label = Some(label);
        }

        if let Some(icon) = thing
            .fields
            .get(&canon::ICON_NAME)
            .and_then(graph::extract_text)
        {
            self.icon = Some(icon);
        }

        if let Some(action) = thing
            .fields
            .get(&canon::ACTION)
            .and_then(graph::extract_text)
        {
            self.action = Some(action);
        }

        if let Some(description) = thing
            .fields
            .get(&canon::DESCRIPTION)
            .and_then(graph::extract_text)
        {
            self.description = Some(description);
        }

        if let Some(focusable) = thing
            .fields
            .get(&canon::FOCUSABLE)
            .and_then(|v| v.as_bool())
        {
            self.focusable = focusable;
        }

        if let Some(tab_index) = thing.fields.get(&canon::TAB_INDEX).and_then(|v| v.as_i64()) {
            self.tab_index = Some(tab_index);
        }

        if let Some(bitmap) = thing
            .fields
            .get(&canon::BITMAP)
            .or_else(|| thing.fields.get(&canon::cc('I', 'D')))
            .and_then(|v| match v {
                Value::Bytes(b) => Some(b.to_vec()),
                _ => None,
            })
        {
            self.bitmap = Some(bitmap);
        }

        if let Some(width) = thing.fields.get(&canon::WIDTH).and_then(|v| v.as_u64()) {
            self.width = Some(width);
        }

        if let Some(height) = thing.fields.get(&canon::HEIGHT).and_then(|v| v.as_u64()) {
            self.height = Some(height);
        }

        if let Some(w) = thing.fields.get(&canon::MIN_WIDTH).and_then(|v| v.as_u64()) {
            self.min_width = Some(w);
        }
        if let Some(w) = thing.fields.get(&canon::MAX_WIDTH).and_then(|v| v.as_u64()) {
            self.max_width = Some(w);
        }
        if let Some(h) = thing
            .fields
            .get(&canon::MIN_HEIGHT)
            .and_then(|v| v.as_u64())
        {
            self.min_height = Some(h);
        }
        if let Some(h) = thing
            .fields
            .get(&canon::MAX_HEIGHT)
            .and_then(|v| v.as_u64())
        {
            self.max_height = Some(h);
        }

        if let Some(x) = thing.fields.get(&canon::X).and_then(|v| v.as_u64()) {
            self.x = Some(x);
        }

        if let Some(y) = thing.fields.get(&canon::Y).and_then(|v| v.as_u64()) {
            self.y = Some(y);
        }

        if let Some(fd) = thing
            .fields
            .get(&canon::cc('F', 'D'))
            .and_then(FlexDirection::from_value)
        {
            self.flex_direction = Some(fd);
        }
        if let Some(jc) = thing
            .fields
            .get(&canon::cc('J', 'C'))
            .and_then(JustifyContent::from_value)
        {
            self.justify_content = Some(jc);
        }
        if let Some(ai) = thing
            .fields
            .get(&canon::cc('A', 'I'))
            .and_then(AlignItems::from_value)
        {
            self.align_items = Some(ai);
        }
        if let Some(fg) = thing
            .fields
            .get(&canon::cc('F', 'G'))
            .and_then(|v| v.as_i64())
        {
            self.flex_grow = Some(fg as f32);
        }
        if let Some(fs) = thing
            .fields
            .get(&canon::cc('F', 'S'))
            .and_then(|v| v.as_i64())
        {
            self.flex_shrink = Some(fs as f32);
        }
        if let Some(gap) = thing.fields.get(&canon::GAP).and_then(|v| v.as_i64()) {
            self.gap = Some(gap as i32);
        }
    }
}

impl Thingable for Widget {
    fn kind() -> &'static str {
        "widget"
    }

    fn load(thing: &GraphThing) -> Option<Self> {
        if thing.kind != canon::WIDGET {
            return None;
        }
        let role = thing
            .fields
            .get(&canon::ROLE)
            .and_then(graph::extract_text)
            .or_else(|| {
                thing
                    .fields
                    .get(&canon::cc('W', 'K'))
                    .and_then(graph::extract_text)
            })
            .unwrap_or_default();
        let kind = thing
            .fields
            .get(&canon::cc('W', 'K'))
            .and_then(graph::extract_text);
        let parent = thing.fields.get(&canon::PARENT).and_then(|v| v.as_uuid());
        let visible = thing
            .fields
            .get(&canon::VISIBLE)
            .and_then(|v| v.as_bool())
            .unwrap_or(true);
        let enabled = thing
            .fields
            .get(&canon::ACTIVE) // Reusing ACTIVE for enabled if appropriate, or define ENABLED
            .and_then(|v| v.as_bool())
            .unwrap_or(true);
        let label = thing
            .fields
            .get(&canon::LABEL)
            .or_else(|| thing.fields.get(&canon::ITEM_LABEL))
            .and_then(graph::extract_text);
        let icon = thing
            .fields
            .get(&canon::ICON_NAME)
            .and_then(graph::extract_text);
        let action = thing
            .fields
            .get(&canon::ACTION)
            .and_then(graph::extract_text);
        let description = thing
            .fields
            .get(&canon::DESCRIPTION)
            .and_then(graph::extract_text);
        let focusable = thing
            .fields
            .get(&canon::FOCUSABLE)
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        let tab_index = thing.fields.get(&canon::TAB_INDEX).and_then(|v| v.as_i64());
        let bitmap = thing
            .fields
            .get(&canon::BITMAP)
            .or_else(|| thing.fields.get(&canon::cc('I', 'D')))
            .and_then(|v| match v {
                Value::Bytes(b) => Some(b.to_vec()),
                _ => None,
            });
        let width = thing.fields.get(&canon::WIDTH).and_then(|v| v.as_u64());
        let height = thing.fields.get(&canon::HEIGHT).and_then(|v| v.as_u64());
        let x = thing.fields.get(&canon::X).and_then(|v| v.as_u64());
        let y = thing.fields.get(&canon::Y).and_then(|v| v.as_u64());
        let flex_direction = thing
            .fields
            .get(&canon::cc('F', 'D'))
            .and_then(FlexDirection::from_value);
        let justify_content = thing
            .fields
            .get(&canon::cc('J', 'C'))
            .and_then(JustifyContent::from_value);
        let align_items = thing
            .fields
            .get(&canon::cc('A', 'I'))
            .and_then(AlignItems::from_value);
        let flex_grow = thing
            .fields
            .get(&canon::cc('F', 'G'))
            .and_then(|v| v.as_i64())
            .map(|v| v as f32);
        let flex_shrink = thing
            .fields
            .get(&canon::cc('F', 'S'))
            .and_then(|v| v.as_i64())
            .map(|v| v as f32);
        let gap = thing
            .fields
            .get(&canon::GAP)
            .and_then(|v| v.as_i64())
            .map(|v| v as i32);

        let min_width = thing.fields.get(&canon::MIN_WIDTH).and_then(|v| v.as_u64());
        let max_width = thing.fields.get(&canon::MAX_WIDTH).and_then(|v| v.as_u64());
        let min_height = thing
            .fields
            .get(&canon::MIN_HEIGHT)
            .and_then(|v| v.as_u64());
        let max_height = thing
            .fields
            .get(&canon::MAX_HEIGHT)
            .and_then(|v| v.as_u64());

        Some(Widget {
            id: thing.id,
            role,
            kind,
            parent,
            visible,
            enabled,
            label,
            icon,
            action,
            description,
            focusable,
            tab_index,
            bitmap,
            width,
            height,
            min_width,
            min_height,
            max_width,
            max_height,
            x,
            y,
            flex_direction,
            justify_content,
            align_items,
            flex_grow,
            flex_shrink,
            gap,
        })
    }
}

#[derive(Clone, Debug)]
pub struct Cursor {
    pub id: Uuid,
    pub kind: String,
    pub position: i64,
    pub page_size: i64,
    pub total_size: i64,
}

impl Thingable for Cursor {
    fn kind() -> &'static str {
        "cursor"
    }

    fn load(thing: &GraphThing) -> Option<Self> {
        if thing.kind != canon::CURSOR {
            return None;
        }
        let kind = thing
            .fields
            .get(&canon::KIND)
            .and_then(graph::extract_text)
            .unwrap_or_default();
        let position = thing
            .fields
            .get(&canon::POSITION)
            .and_then(|v| v.as_i64())
            .unwrap_or(0);
        let page_size = thing
            .fields
            .get(&canon::PAGE_SIZE)
            .and_then(|v| v.as_i64())
            .unwrap_or(0);
        let total_size = thing
            .fields
            .get(&canon::TOTAL_SIZE)
            .and_then(|v| v.as_i64())
            .unwrap_or(0);

        Some(Cursor {
            id: thing.id,
            kind,
            position,
            page_size,
            total_size,
        })
    }
}

pub fn create_widget(role: &str, visible: bool, enabled: bool, label: Option<&str>) -> Uuid {
    let mut fields = graph::map();
    fields.insert(canon::ROLE, Value::Text(String::from(role)));
    fields.insert(canon::VISIBLE, Value::Bool(visible));
    fields.insert(canon::ACTIVE, Value::Bool(enabled)); // Using ACTIVE for enabled
    if let Some(l) = label {
        fields.insert(canon::LABEL, Value::Text(String::from(l)));
    }

    graph::fiat(None, canon::WIDGET, fields)
}

pub fn create_cursor(kind: &str, position: i64, page_size: i64, total_size: i64) -> Uuid {
    let mut fields = graph::map();
    fields.insert(canon::KIND, Value::Text(String::from(kind)));
    fields.insert(canon::POSITION, Value::I64(position));
    fields.insert(canon::PAGE_SIZE, Value::I64(page_size));
    fields.insert(canon::TOTAL_SIZE, Value::I64(total_size));

    graph::fiat(None, canon::CURSOR, fields)
}

pub struct WidgetBuilder<'a, 'b> {
    ctx: &'b mut AppContext<'a>,
    role: String,
    parent: Option<Uuid>,
    label: Option<String>,
    icon: Option<String>,
    action: Option<String>,
    focusable: bool,
}

impl<'a, 'b> WidgetBuilder<'a, 'b> {
    pub fn new(ctx: &'b mut AppContext<'a>) -> Self {
        Self {
            ctx,
            role: "widget".into(),
            parent: None,
            label: None,
            icon: None,
            action: None,
            focusable: false,
        }
    }

    pub fn role(mut self, role: &str) -> Self {
        self.role = role.into();
        self
    }

    pub fn child_of(mut self, parent: Uuid) -> Self {
        self.parent = Some(parent);
        self
    }

    pub fn set_text_label(mut self, label: &str) -> Self {
        self.label = Some(label.into());
        self
    }

    pub fn set_property(mut self, key: Symbol, value: Value) -> Self {
        if key == canon::ICON_NAME {
            if let Value::Text(s) = value {
                self.icon = Some(s);
            }
        } else if key == canon::ACTION {
            if let Value::Text(s) = value {
                self.action = Some(s);
            }
        }
        self
    }

    pub fn focusable(mut self, focusable: bool) -> Self {
        self.focusable = focusable;
        self
    }

    pub fn build(self) -> Uuid {
        let mut fields = graph::map();
        fields.insert(canon::ROLE, Value::Text(self.role));
        if let Some(l) = self.label {
            fields.insert(canon::LABEL, Value::Text(l));
        }
        if let Some(i) = self.icon {
            fields.insert(canon::ICON_NAME, Value::Text(i));
        }
        if let Some(a) = self.action {
            fields.insert(canon::ACTION, Value::Text(a));
        }
        if let Some(parent) = self.parent {
            fields.insert(canon::PARENT, Value::Uuid(parent));
        }
        fields.insert(canon::FOCUSABLE, Value::Bool(self.focusable));

        let widget_id = graph::fiat(None, canon::WIDGET, fields);

        if let Some(parent) = self.parent {
            graph::that(parent, canon::CHILD, widget_id, 0);
        }

        widget_id
    }
}

pub fn toolbar(ctx: &mut AppContext, parent: Uuid) -> Uuid {
    WidgetBuilder::new(ctx)
        .role("container.toolbar")
        .child_of(parent)
        .build()
}

pub fn toolbar_button(
    ctx: &mut AppContext,
    parent: Uuid,
    icon_name: &str,
    action: &str,
    label: Option<&str>,
) -> Uuid {
    let mut b = WidgetBuilder::new(ctx);
    b = b
        .role("control.toolbar_button")
        .child_of(parent)
        .focusable(true)
        .set_property(canon::ICON_NAME, Value::Text(icon_name.into()))
        .set_property(canon::ACTION, Value::Text(action.into()));

    if let Some(l) = label {
        b = b.set_text_label(l);
    }
    b.build()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::canon;
    use crate::graph;
    use crate::runtime::host_runtime;
    use crate::runtime::set_runtime;
    use thing_host::HostRuntime;

    #[test]
    fn test_ui_graph() {
        // Initialize host runtime for testing
        let runtime = HostRuntime::new();
        set_runtime(runtime);

        // Create a window widget
        let window_id = create_widget("window", true, true, Some("Test Window"));

        // Create a scroll container
        let scroll_container_id = create_widget("scroll_container", true, true, None);
        graph::that(window_id, canon::CHILD, scroll_container_id, 0);

        // Create content
        let content_id = create_widget("list", true, true, None);
        graph::that(scroll_container_id, canon::HAS_CONTENT, content_id, 0);

        // Create cursor
        let cursor_id = create_cursor("vertical", 0, 100, 1000);
        graph::that(scroll_container_id, canon::HAS_CURSOR, cursor_id, 0);

        // Create scrollbar
        let scrollbar_id = create_widget("scrollbar", true, true, None);
        graph::that(window_id, canon::CHILD, scrollbar_id, 0);
        graph::that(scrollbar_id, canon::CONTROLS, cursor_id, 0);

        // Verify the graph structure
        let window_thing = graph::load_thing::<Widget>(window_id).expect("Window not found");
        assert_eq!(window_thing.role, "window");
        assert_eq!(window_thing.label.as_deref(), Some("Test Window"));

        let cursor_thing = graph::load_thing::<Cursor>(cursor_id).expect("Cursor not found");
        assert_eq!(cursor_thing.kind, "vertical");
        assert_eq!(cursor_thing.position, 0);
        assert_eq!(cursor_thing.total_size, 1000);

        // In a real test we would query edges too, but load_thing only checks the node.
        // This confirms the schema is valid and can be stored/retrieved.
    }
}
