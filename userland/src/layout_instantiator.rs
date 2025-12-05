use crate::flex::{AlignItems, FlexDirection, JustifyContent};
use crate::questions::AnswerKind;
use crate::{canon, graph, Symbol, Value};
use alloc::collections::BTreeMap;
use alloc::string::{String, ToString};
use alloc::vec::Vec;
use uuid::Uuid;

pub struct LayoutInstantiator;

impl LayoutInstantiator {
    pub fn instantiate(
        window_id: Uuid,
        template_id: Uuid,
        bindings: &BTreeMap<String, Uuid>,
        bundle_id: Uuid,
    ) {
        // 1. Read the template node to get the root regions
        let template_props = graph::get_props(graph::GraphPropsGetRequest {
            node: template_id,
            keys: alloc::vec![canon::CONTAINS],
        });

        if let Some(props) = template_props {
            if let Some(Value::List(regions)) = props.get(&canon::CONTAINS) {
                for region_val in regions {
                    if let Value::Uuid(region_id) = region_val {
                        Self::instantiate_region(window_id, None, *region_id, bindings, bundle_id);
                    }
                }
            }
        }
    }

    fn instantiate_region(
        window_id: Uuid,
        parent_widget_id: Option<Uuid>,
        region_id: Uuid,
        bindings: &BTreeMap<String, Uuid>,
        bundle_id: Uuid,
    ) {
        // Read region properties
        let req = graph::GraphPropsGetRequest {
            node: region_id,
            keys: alloc::vec![
                canon::ROLE,
                canon::WIDGET_KIND,
                canon::WIDTH,
                canon::HEIGHT,
                canon::GAP,
                canon::FLEX_DIRECTION,
                canon::FLEX_GROW,
                canon::FLEX_SHRINK,
                canon::BINDS_TO,
                canon::TEXT,
                canon::ICON_NAME,
                canon::CONTAINS,
                canon::FOCUSABLE,
                canon::TARGET,
                canon::COLOR,
                canon::SHOW_TAG,
            ],
        };

        let Some(props) = graph::get_props(req) else {
            return;
        };

        // Determine widget kind from explicit prop or role
        let role = props.get(&canon::ROLE).and_then(|v| v.as_text());

        let widget_kind_str = if let Some(Value::Text(k)) = props.get(&canon::WIDGET_KIND) {
            k.clone()
        } else if let Some(r) = role {
            match r {
                "control.toolbar_button" => "button".to_string(),
                "window_root" => "container".to_string(),
                "listbox_default" => "listbox".to_string(),
                "graph_mini_viewer" => "graph_mini_viewer".to_string(),
                "thing_inspector" => "thing_inspector".to_string(),
                "top_status_bar" => "top_status_bar".to_string(),
                "image" => "image".to_string(),
                _ => "container".to_string(),
            }
        } else {
            "container".to_string()
        };

        // Create the widget
        let seed = alloc::format!("{}-{}-widget", window_id, region_id);
        let widget_id = crate::simple_uuid(seed.as_bytes());
        let mut widget_props = BTreeMap::new();

        // Basic props
        widget_props.insert(canon::KIND, Value::Symbol(canon::WIDGET));
        widget_props.insert(canon::cc('W', 'K'), Value::Text(widget_kind_str));
        widget_props.insert(
            canon::PARENT,
            Value::Uuid(parent_widget_id.unwrap_or(window_id)),
        );
        widget_props.insert(canon::BUNDLE_ID, Value::Uuid(bundle_id));

        // Apply role-based defaults
        if let Some(r) = role {
            match r {
                "control.toolbar_button" => {
                    // Default styling for toolbar buttons
                    widget_props.entry(canon::GAP).or_insert(Value::I64(4));
                    // Could add padding, etc. if we had symbols for them
                }
                "window_root" => {
                    widget_props
                        .entry(canon::cc('F', 'D'))
                        .or_insert(Value::Text("column".to_string()));
                }
                _ => {}
            }
        }

        // Copy layout props (overriding defaults)
        if let Some(v) = props.get(&canon::WIDTH) {
            widget_props.insert(canon::WIDTH, v.clone());
        }
        if let Some(v) = props.get(&canon::HEIGHT) {
            widget_props.insert(canon::HEIGHT, v.clone());
        }
        if let Some(v) = props.get(&canon::GAP) {
            widget_props.insert(canon::GAP, v.clone());
        }
        if let Some(v) = props.get(&canon::FLEX_DIRECTION) {
            widget_props.insert(canon::cc('F', 'D'), v.clone());
        }
        if let Some(v) = props.get(&canon::FLEX_GROW) {
            widget_props.insert(canon::cc('F', 'G'), v.clone());
        }
        if let Some(v) = props.get(&canon::FLEX_SHRINK) {
            widget_props.insert(canon::cc('F', 'S'), v.clone());
        }
        if let Some(v) = props.get(&canon::ROLE) {
            widget_props.insert(canon::ROLE, v.clone());
        }
        if let Some(v) = props.get(&canon::TEXT) {
            widget_props.insert(canon::TEXT, v.clone());
        }
        if let Some(v) = props.get(&canon::ICON_NAME) {
            widget_props.insert(canon::ICON_NAME, v.clone());
        }
        if let Some(v) = props.get(&canon::FOCUSABLE) {
            widget_props.insert(canon::FOCUSABLE, v.clone());
        }
        if let Some(v) = props.get(&canon::TARGET) {
            widget_props.insert(canon::TARGET, v.clone());
        }
        if let Some(v) = props.get(&canon::COLOR) {
            widget_props.insert(canon::COLOR, v.clone());
        }

        // Handle binding
        if let Some(Value::Text(bind_name)) = props.get(&canon::BINDS_TO) {
            if let Some(target_node) = bindings.get(bind_name) {
                widget_props.insert(canon::BINDS, Value::Uuid(*target_node));
            }
        }

        // Create the widget node
        graph::fiat(Some(widget_id), canon::WIDGET, widget_props);

        // Handle dynamic population
        if let Some(Value::Text(tag)) = props.get(&canon::SHOW_TAG) {
            Self::instantiate_tagged_questions(window_id, widget_id, tag, bundle_id);
        }

        // Recurse for children
        if let Some(Value::List(children)) = props.get(&canon::CONTAINS) {
            for child_val in children {
                if let Value::Uuid(child_id) = child_val {
                    Self::instantiate_region(
                        window_id,
                        Some(widget_id),
                        *child_id,
                        bindings,
                        bundle_id,
                    );
                }
            }
        }
    }

    fn instantiate_tagged_questions(window_id: Uuid, parent_id: Uuid, tag: &str, bundle_id: Uuid) {
        let mut props_map = BTreeMap::new();
        props_map.insert(canon::TAG, Value::Text(tag.to_string()));

        let pattern = graph::NodePattern {
            labels: alloc::vec![canon::QUESTION],
            props: props_map,
        };

        let questions = graph::get_nodes(pattern);

        for question in questions {
            let answer_kind = question
                .fields
                .get(&canon::ANSWER_KIND)
                .and_then(AnswerKind::from_value);
            let preferred_role = question
                .fields
                .get(&canon::PREFERRED_WIDGET)
                .and_then(|v| v.as_text());

            let widget_kind = if let Some(role) = preferred_role {
                match role {
                    "toggle" => "checkbox",
                    "dropdown" => "dropdown",
                    "slider" => "slider",
                    _ => "label",
                }
            } else {
                match answer_kind {
                    Some(AnswerKind::YesNo) => "checkbox",
                    Some(AnswerKind::Text) => "text_entry",
                    Some(AnswerKind::OneOf) => "dropdown",
                    Some(AnswerKind::Number) => "label",
                    None => "label",
                }
            };

            let seed = alloc::format!("{}-{}-qwidget", window_id, question.id);
            let widget_id = crate::simple_uuid(seed.as_bytes());
            let mut widget_props = BTreeMap::new();

            widget_props.insert(canon::KIND, Value::Symbol(canon::WIDGET));
            widget_props.insert(canon::cc('W', 'K'), Value::Text(widget_kind.to_string()));
            widget_props.insert(canon::PARENT, Value::Uuid(parent_id));
            widget_props.insert(canon::BUNDLE_ID, Value::Uuid(bundle_id));

            if let Some(label) = question
                .fields
                .get(&canon::NAME)
                .or(question.fields.get(&canon::TEXT))
            {
                widget_props.insert(canon::TEXT, label.clone());
            }

            // Bind to the question node itself
            widget_props.insert(canon::BINDS, Value::Uuid(question.id));

            // Default styling
            widget_props.insert(canon::WIDTH, Value::I64(200));
            widget_props.insert(canon::HEIGHT, Value::I64(30));
            widget_props.insert(canon::FOCUSABLE, Value::Bool(true));

            graph::fiat(Some(widget_id), canon::WIDGET, widget_props);
        }
    }
}
