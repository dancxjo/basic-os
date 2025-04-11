use alloc::{boxed::Box, string::ToString, vec::Vec};

use crate::thing::Thingable;

#[derive(Debug)]
pub struct Message {
    pub text: &'static str,
}

impl Thingable for Message {
    fn kind() -> &'static str {
        "message"
    }

    fn serialize(&self) -> Vec<u8> {
        self.text.as_bytes().to_vec()
    }

    fn deserialize(bytes: &[u8]) -> Option<Self> {
        core::str::from_utf8(bytes).ok().map(|s| Message {
            text: Box::leak(s.to_string().into_boxed_str()),
        })
    }
}
