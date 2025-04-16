use serde::{Deserialize, Serialize};
use thing_macros::Thing;

use crate::thing::Thingable;
use alloc::string::{String, ToString};

#[derive(Debug, Serialize, Deserialize, Thing)]
pub struct Message {
    pub text: String,
}

impl Message {
    pub fn new(text: &str) -> Self {
        Message {
            text: text.to_string(),
        }
    }
}
