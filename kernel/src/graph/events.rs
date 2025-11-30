use crate::graph::canon;
use crate::graph::journal;
use crate::graph::types::{GraphEdge, GraphThing};
use alloc::collections::BTreeMap;
use alloc::string::String;
use thing_abi::Value;

pub(crate) fn emit_thing_event(thing: &GraphThing) {
    let mut payload = BTreeMap::new();
    payload.insert(canon::ID, Value::Uuid(thing.id));
    payload.insert(canon::KIND, Value::Symbol(thing.kind));
    payload.insert(canon::FIELDS, Value::Map(thing.fields.clone()));
    payload.insert(canon::OWNER, Value::Uuid(thing.owner));
    payload.insert(canon::REVISION, Value::U64(thing.revision));
    let _ = journal::emit_data(canon::THING_CREATED, Value::Map(payload));
}

pub(crate) fn emit_edge_event(edge: &GraphEdge) {
    let mut payload = BTreeMap::new();
    payload.insert(canon::SRC, Value::Uuid(edge.src));
    payload.insert(canon::DST, Value::Uuid(edge.dst));
    payload.insert(canon::PREDICATE, Value::Text(edge.pred.clone()));
    payload.insert(canon::OWNER, Value::Uuid(edge.owner));
    if !edge.props.is_empty() {
        payload.insert(canon::FIELDS, Value::Map(edge.props.clone()));
    }
    payload.insert(canon::REVISION, Value::U64(edge.revision));
    let _ = journal::emit_data(canon::EDGE_ADDED, Value::Map(payload));
}

pub(crate) fn reflect_thing_side_effects(thing: &GraphThing) {
    if thing.kind != canon::WRITE {
        return;
    }

    // Console writing has been moved to userland.
    // This function can be extended for other kernel-side side effects if needed.
}

pub(crate) fn extract_text(value: &Value) -> Option<String> {
    match value {
        Value::Text(s) => Some(s.clone()),
        Value::Bytes(b) => core::str::from_utf8(b).ok().map(String::from),
        Value::Map(m) => m.get(&canon::TEXT).and_then(extract_text),
        _ => None,
    }
}
