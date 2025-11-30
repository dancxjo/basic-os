use crate::{canon, graph, Thingable, Value};
use alloc::string::String;
use thing_abi::GraphThing;
use uuid::Uuid;

#[derive(Clone, Debug)]
pub struct Widget {
    pub id: Uuid,
    pub role: String,
    pub visible: bool,
    pub enabled: bool,
    pub label: Option<String>,
    pub description: Option<String>,
    pub focusable: bool,
    pub tab_index: Option<i64>,
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
            .unwrap_or_default();
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

        Some(Widget {
            id: thing.id,
            role,
            visible,
            enabled,
            label,
            description,
            focusable,
            tab_index,
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graph;
    use crate::runtime::host_runtime;
    use crate::runtime::set_runtime;
    use thing_host::HostRuntime;

    #[test]
    fn test_semantic_ui_graph() {
        // Initialize host runtime for testing
        let runtime = HostRuntime::new();
        set_runtime(runtime);

        // Create a window widget
        let window_id = create_widget("window", true, true, Some("Test Window"));

        // Create a scroll container
        let scroll_container_id = create_widget("scroll_container", true, true, None);
        graph::that(window_id, "CHILD", scroll_container_id, 0);

        // Create content
        let content_id = create_widget("list", true, true, None);
        graph::that(scroll_container_id, "HAS_CONTENT", content_id, 0);

        // Create cursor
        let cursor_id = create_cursor("vertical", 0, 100, 1000);
        graph::that(scroll_container_id, "HAS_CURSOR", cursor_id, 0);

        // Create scrollbar
        let scrollbar_id = create_widget("scrollbar", true, true, None);
        graph::that(window_id, "CHILD", scrollbar_id, 0);
        graph::that(scrollbar_id, "CONTROLS", cursor_id, 0);

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
