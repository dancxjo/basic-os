use alloc::string::{String, ToString};

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
