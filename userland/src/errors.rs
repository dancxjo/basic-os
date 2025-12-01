use crate::{canon, graph, Value};
use alloc::format;

pub fn notify_error(message: &str, details: &str) {
    let mut fields = graph::map();
    fields.insert(canon::KIND, Value::Symbol(canon::NOTIFICATION));
    fields.insert(canon::LEVEL, Value::Text("error".into()));
    fields.insert(canon::MESSAGE, Value::Text(message.into()));
    fields.insert(canon::DETAILS, Value::Text(details.into()));
    fields.insert(canon::SCOPE, Value::Text("local".into()));
    fields.insert(canon::ACK, Value::Bool(false));
    // For now, we don't have a reliable clock in userland without syscalls,
    // so we'll skip CREATED_AT or use a placeholder.
    // fields.insert(canon::CREATED_AT, Value::Text(now_iso8601()?));
    graph::fiat(None, canon::NOTIFICATION, fields);
}

pub fn with_notification<T, E: core::fmt::Debug>(
    label: &str,
    f: impl FnOnce() -> Result<T, E>,
) -> Option<T> {
    match f() {
        Ok(v) => Some(v),
        Err(e) => {
            notify_error(label, &format!("{:?}", e));
            None
        }
    }
}

pub fn raise_global_alert(message: &str, details: &str) {
    let mut fields = graph::map();
    fields.insert(canon::KIND, Value::Symbol(canon::NOTIFICATION));
    fields.insert(canon::LEVEL, Value::Text("error".into()));
    fields.insert(canon::MESSAGE, Value::Text(message.into()));
    fields.insert(canon::DETAILS, Value::Text(details.into()));
    fields.insert(canon::SCOPE, Value::Text("global".into()));
    fields.insert(canon::PERSIST, Value::Bool(true));
    fields.insert(canon::ACK, Value::Bool(false));
    graph::fiat(None, canon::NOTIFICATION, fields);
}

#[macro_export]
macro_rules! notify_error {
    ($msg:expr, $details:expr) => {
        $crate::errors::notify_error($msg, $details);
    };
    ($msg:expr) => {
        $crate::errors::notify_error($msg, "");
    };
}
