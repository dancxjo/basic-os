use crate::graph::GraphThing;
use crate::{canon, graph, Value};
use thing_abi::GraphPropsRequest;
use uuid::Uuid;

/// Semantic descriptor for the shape of an answer.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AnswerKind {
    YesNo,
    OneOf,
    Text,
    Number,
}

impl AnswerKind {
    pub fn to_value(self) -> Value {
        match self {
            AnswerKind::YesNo => Value::Text("yes_no".into()),
            AnswerKind::OneOf => Value::Text("one_of".into()),
            AnswerKind::Text => Value::Text("text".into()),
            AnswerKind::Number => Value::Text("number".into()),
        }
    }

    pub fn from_value(value: &Value) -> Option<Self> {
        match value {
            Value::Text(t) => match t.as_str() {
                "yes_no" => Some(AnswerKind::YesNo),
                "one_of" => Some(AnswerKind::OneOf),
                "text" => Some(AnswerKind::Text),
                "number" => Some(AnswerKind::Number),
                _ => None,
            },
            _ => None,
        }
    }
}

/// Typed wrapper for values a question can hold.
#[derive(Clone, Debug, PartialEq)]
pub enum AnswerValue {
    Bool(bool),
    Text(String),
    Number(i64),
}

impl AnswerValue {
    pub fn kind(&self) -> AnswerKind {
        match self {
            AnswerValue::Bool(_) => AnswerKind::YesNo,
            AnswerValue::Text(_) => AnswerKind::Text,
            AnswerValue::Number(_) => AnswerKind::Number,
        }
    }
}

/// Connection between a question and its answer Thing.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct QuestionBinding {
    pub question: Uuid,
    pub answer: Uuid,
    pub kind: AnswerKind,
}

impl QuestionBinding {
    /// Update the backing answer Thing with a new value.
    pub fn set_answer(&self, value: AnswerValue) -> bool {
        let mut props = graph::map();
        match value {
            AnswerValue::Bool(v) => {
                props.insert(canon::VALUE_BOOL, Value::Bool(v));
            }
            AnswerValue::Text(v) => {
                props.insert(canon::VALUE_TEXT, Value::Text(v));
            }
            AnswerValue::Number(v) => {
                props.insert(canon::VALUE_NUMBER, Value::I64(v));
            }
        }

        graph::set_props(GraphPropsRequest {
            node: self.answer,
            props,
        })
    }

    /// Try to interpret a graph Thing as the latest answer value.
    pub fn read_answer(thing: &GraphThing) -> Option<AnswerValue> {
        if thing.kind != canon::ANSWER {
            return None;
        }

        if let Some(v) = thing
            .fields
            .get(&canon::VALUE_BOOL)
            .and_then(|v| v.as_bool())
        {
            return Some(AnswerValue::Bool(v));
        }

        if let Some(v) = thing
            .fields
            .get(&canon::VALUE_TEXT)
            .and_then(crate::graph::extract_text)
        {
            return Some(AnswerValue::Text(v));
        }

        if let Some(v) = thing
            .fields
            .get(&canon::VALUE_NUMBER)
            .and_then(|v| v.as_i64())
        {
            return Some(AnswerValue::Number(v));
        }

        None
    }
}

/// Create a form container Thing.
///
/// The resulting Thing is labelled `FORM` and can be linked to questions via
/// `FORM_CONTAINS` edges.
pub fn ensure_form(id: Option<Uuid>, name: &str) -> Uuid {
    let mut fields = graph::map();
    fields.insert(canon::LABEL, Value::Text(name.into()));
    graph::fiat(id, canon::FORM, fields)
}

/// Create or update a QUESTION and its ANSWER, linking them automatically.
///
/// # Example
///
/// ```
/// use userland::questions::{ensure_question_with_answer, AnswerKind, AnswerValue};
/// use userland::uuid::Uuid;
///
/// let (question_id, answer_id, binding) = ensure_question_with_answer(
///     None,
///     None,
///     "Enable telemetry?",
///     None,
///     AnswerValue::Bool(false),
/// );
/// assert_eq!(binding.question, question_id);
/// assert_eq!(binding.answer, answer_id);
/// assert_eq!(binding.kind, AnswerKind::YesNo);
/// ```
pub fn ensure_question_with_answer(
    question_id: Option<Uuid>,
    answer_id: Option<Uuid>,
    label: &str,
    description: Option<&str>,
    initial_value: AnswerValue,
) -> (Uuid, Uuid, QuestionBinding) {
    let question =
        question_id.unwrap_or_else(|| Uuid::new_v5(&Uuid::NAMESPACE_OID, label.as_bytes()));
    let answer = answer_id.unwrap_or_else(|| {
        let mut salt = label.as_bytes().to_vec();
        salt.extend_from_slice(b"::answer");
        Uuid::new_v5(&Uuid::NAMESPACE_OID, &salt)
    });

    let mut q_fields = graph::map();
    q_fields.insert(canon::LABEL, Value::Text(label.into()));
    q_fields.insert(canon::ANSWER_KIND, initial_value.kind().to_value());
    if let Some(desc) = description {
        q_fields.insert(canon::DESCRIPTION, Value::Text(desc.into()));
    }
    graph::fiat(Some(question), canon::QUESTION, q_fields);

    let mut a_fields = graph::map();
    match &initial_value {
        AnswerValue::Bool(v) => {
            a_fields.insert(canon::VALUE_BOOL, Value::Bool(*v));
        }
        AnswerValue::Text(v) => {
            a_fields.insert(canon::VALUE_TEXT, Value::Text(v.clone()));
        }
        AnswerValue::Number(v) => {
            a_fields.insert(canon::VALUE_NUMBER, Value::I64(*v));
        }
    }
    graph::fiat(Some(answer), canon::ANSWER, a_fields);
    graph::that(question, canon::HAS_ANSWER, answer, 0);

    (
        question,
        answer,
        QuestionBinding {
            question,
            answer,
            kind: initial_value.kind(),
        },
    )
}

/// Link a question into a form using `FORM_CONTAINS`.
pub fn attach_question_to_form(form: Uuid, question: Uuid) {
    graph::that(form, canon::FORM_CONTAINS, question, 0);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn answer_kind_round_trips() {
        for (kind, text) in [
            (AnswerKind::YesNo, "yes_no"),
            (AnswerKind::OneOf, "one_of"),
            (AnswerKind::Text, "text"),
            (AnswerKind::Number, "number"),
        ] {
            assert_eq!(
                AnswerKind::from_value(&Value::Text(text.into())),
                Some(kind)
            );
            assert_eq!(kind.to_value(), Value::Text(text.into()));
        }
    }
}
