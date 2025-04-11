use serde::{Deserialize, Serialize};
use thing_macros::Thing;

use crate::thing::Thingable;
use alloc::string::String;

#[derive(Debug, Serialize, Deserialize, Thing)]
pub struct Message {
    pub text: String,
}
