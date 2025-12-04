use crate::graph::GraphThing;
use crate::{canon, graph, Value};
use alloc::string::String;
use thing_abi::GraphPropsRequest;
use uuid::Uuid;

use crate::questions::{AnswerKind, QuestionBinding};

/// Interaction kinds describe how a widget converses with the graph.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InteractionKind {
    Inspect,
    Select,
    DecideBool,
    DecideOneOf,
    AdjustNumber,
    AdjustText,
    Act,
    Navigate,
    Arrange,
    Annotate,
}

impl InteractionKind {
    fn as_str(self) -> &'static str {
        match self {
            InteractionKind::Inspect => "inspect",
            InteractionKind::Select => "select",
            InteractionKind::DecideBool => "decide_bool",
            InteractionKind::DecideOneOf => "decide_one_of",
            InteractionKind::AdjustNumber => "adjust_number",
            InteractionKind::AdjustText => "adjust_text",
            InteractionKind::Act => "act",
            InteractionKind::Navigate => "navigate",
            InteractionKind::Arrange => "arrange",
            InteractionKind::Annotate => "annotate",
        }
    }

    /// Serialize the kind into a graph-friendly value.
    pub fn to_value(self) -> Value {
        Value::Text(self.as_str().into())
    }

    /// Try to parse an interaction kind from a Value.
    pub fn from_value(value: &Value) -> Option<Self> {
        match value {
            Value::Text(s) => match s.as_str() {
                "inspect" => Some(InteractionKind::Inspect),
                "select" => Some(InteractionKind::Select),
                "decide_bool" => Some(InteractionKind::DecideBool),
                "decide_one_of" => Some(InteractionKind::DecideOneOf),
                "adjust_number" => Some(InteractionKind::AdjustNumber),
                "adjust_text" => Some(InteractionKind::AdjustText),
                "act" => Some(InteractionKind::Act),
                "navigate" => Some(InteractionKind::Navigate),
                "arrange" => Some(InteractionKind::Arrange),
                "annotate" => Some(InteractionKind::Annotate),
                _ => None,
            },
            _ => None,
        }
    }
}

/// Graph links describing how a widget and semantic target are bound together.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct InteractionBinding {
    pub interaction: Uuid,
    pub kind: InteractionKind,
    pub target: Uuid,
    pub result: Option<Uuid>,
    pub widget: Option<Uuid>,
}

impl InteractionBinding {
    /// Create (or update) an interaction node and edges describing the binding.
    pub fn ensure(
        id: Option<Uuid>,
        kind: InteractionKind,
        target: Uuid,
        result: Option<Uuid>,
        widget: Option<Uuid>,
        label: Option<&str>,
        description: Option<&str>,
    ) -> Self {
        let mut props = graph::map();
        props.insert(canon::INTERACTION_KIND, kind.to_value());
        props.insert(canon::TARGET, Value::Uuid(target));

        if let Some(label) = label {
            props.insert(canon::LABEL, Value::Text(label.into()));
        }
        if let Some(desc) = description {
            props.insert(canon::DESCRIPTION, Value::Text(desc.into()));
        }

        let interaction = graph::fiat(id, canon::INTERACTION, props);
        graph::that(interaction, canon::TARGET, target, 0);
        if let Some(result) = result {
            graph::set_props(GraphPropsRequest {
                node: interaction,
                props: {
                    let mut props = graph::map();
                    props.insert(canon::RESULT_WRITES, Value::Uuid(result));
                    props
                },
            });
            graph::that(interaction, canon::RESULT_WRITES, result, 0);
        }
        if let Some(widget_id) = widget {
            graph::set_props(GraphPropsRequest {
                node: interaction,
                props: {
                    let mut props = graph::map();
                    props.insert(canon::USES_WIDGET, Value::Uuid(widget_id));
                    props
                },
            });
            graph::that(interaction, canon::USES_WIDGET, widget_id, 0);
            graph::that(widget_id, canon::CONTROLS, interaction, 0);
        }

        InteractionBinding {
            interaction,
            kind,
            target,
            result,
            widget,
        }
    }

    /// Interpret a graph Thing as an interaction description.
    pub fn load(thing: &GraphThing, widget: Option<Uuid>) -> Option<Self> {
        if thing.kind != canon::INTERACTION {
            return None;
        }
        let Some(kind_value) = thing.fields.get(&canon::INTERACTION_KIND) else {
            return None;
        };
        let kind = InteractionKind::from_value(kind_value)?;
        let target = thing.fields.get(&canon::TARGET)?.as_uuid()?;
        let result = thing
            .fields
            .get(&canon::RESULT_WRITES)
            .and_then(|v| v.as_uuid());

        Some(InteractionBinding {
            interaction: thing.id,
            kind,
            target,
            result,
            widget,
        })
    }
}

/// Helper for binding a `QuestionBinding` to an interaction node.
///
/// A yes/no question becomes a `decide_bool` interaction, a one-of question is
/// `decide_one_of`, numeric answers use `adjust_number`, and text answers use
/// `adjust_text`.
///
/// # Examples
///
/// ```
/// use userland::interactions::{ensure_question_interaction, InteractionKind};
/// use userland::questions::{AnswerKind, QuestionBinding};
/// use userland::uuid::Uuid;
///
/// let binding = QuestionBinding {
///     question: Uuid::nil(),
///     answer: Uuid::nil(),
///     kind: AnswerKind::YesNo,
/// };
/// let interaction = ensure_question_interaction(&binding, None, Some("Toggle"), None);
/// assert_eq!(interaction.kind, InteractionKind::DecideBool);
/// ```
pub fn ensure_question_interaction(
    binding: &QuestionBinding,
    widget: Option<Uuid>,
    label: Option<&str>,
    description: Option<&str>,
) -> InteractionBinding {
    let kind = match binding.kind {
        AnswerKind::YesNo => InteractionKind::DecideBool,
        AnswerKind::OneOf => InteractionKind::DecideOneOf,
        AnswerKind::Number => InteractionKind::AdjustNumber,
        AnswerKind::Text => InteractionKind::AdjustText,
    };

    InteractionBinding::ensure(
        None,
        kind,
        binding.question,
        Some(binding.answer),
        widget,
        label,
        description,
    )
}

/// Convenience for wiring a button-style widget to an action target.
///
/// ```
/// use userland::interactions::{ensure_action_interaction, InteractionKind};
/// use userland::uuid::Uuid;
///
/// let binding = ensure_action_interaction(Uuid::nil(), None, Some("Save"), None);
/// assert_eq!(binding.kind, InteractionKind::Act);
/// ```
pub fn ensure_action_interaction(
    target: Uuid,
    widget: Option<Uuid>,
    label: Option<&str>,
    description: Option<&str>,
) -> InteractionBinding {
    InteractionBinding::ensure(
        None,
        InteractionKind::Act,
        target,
        None,
        widget,
        label,
        description,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graph;
    use crate::runtime::set_runtime;
    use thing_host::HostRuntime;

    #[test]
    fn interaction_kind_round_trips() {
        for (kind, text) in [
            (InteractionKind::Inspect, "inspect"),
            (InteractionKind::Select, "select"),
            (InteractionKind::DecideBool, "decide_bool"),
            (InteractionKind::DecideOneOf, "decide_one_of"),
            (InteractionKind::AdjustNumber, "adjust_number"),
            (InteractionKind::AdjustText, "adjust_text"),
            (InteractionKind::Act, "act"),
            (InteractionKind::Navigate, "navigate"),
            (InteractionKind::Arrange, "arrange"),
            (InteractionKind::Annotate, "annotate"),
        ] {
            assert_eq!(
                InteractionKind::from_value(&Value::Text(text.into())),
                Some(kind)
            );
            assert_eq!(kind.to_value(), Value::Text(text.into()));
        }
    }

    #[test]
    fn question_binding_creates_interaction() {
        let runtime = HostRuntime::new();
        set_runtime(runtime);

        let question = graph::fiat(None, canon::QUESTION, graph::map());
        let answer = graph::fiat(None, canon::ANSWER, graph::map());
        let widget = graph::fiat(None, canon::WIDGET, graph::map());
        let binding = QuestionBinding {
            question,
            answer,
            kind: AnswerKind::YesNo,
        };

        let interaction = ensure_question_interaction(&binding, Some(widget), Some("Label"), None);

        let loaded = graph::load_thing::<GraphThing>(interaction.interaction)
            .expect("interaction should exist");
        assert_eq!(
            loaded
                .fields
                .get(&canon::INTERACTION_KIND)
                .and_then(InteractionKind::from_value),
            Some(InteractionKind::DecideBool)
        );
        assert_eq!(
            loaded
                .fields
                .get(&canon::LABEL)
                .and_then(graph::extract_text),
            Some(String::from("Label"))
        );

        let loaded_binding = InteractionBinding::load(&loaded, Some(widget)).unwrap();
        assert_eq!(loaded_binding.widget, Some(widget));
        assert_eq!(loaded_binding.result, Some(answer));
    }
}
