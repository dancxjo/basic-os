use alloc::{borrow::ToOwned, boxed::Box, format, string::String};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Verb {
    pub name: &'static str,             // "eat"
    pub active_template: &'static str,  // "{} eats {}"
    pub passive_template: &'static str, // "{} is eaten by {}"
    pub subject_role: &'static str,     // "eater"
    pub object_role: &'static str,      // "food"
}

impl Verb {
    pub fn from_regular(bare_form: &str) -> Self {
        let active_template = format!("{{}} {} {{}}", Self::conjugate(bare_form)).into_boxed_str();
        let passive_template =
            format!("{{}} is {} by {{}}", Self::past_participle(bare_form)).into_boxed_str();
        let subject_role = "subject";
        let object_role = "object";

        Self {
            name: Box::leak(bare_form.to_owned().into_boxed_str()),
            active_template: Box::leak(active_template),
            passive_template: Box::leak(passive_template),
            subject_role,
            object_role,
        }
    }

    pub fn conjugate(base: &str) -> String {
        // Naive English 3rd person conjugation
        if base.ends_with("s")
            || base.ends_with("x")
            || base.ends_with("z")
            || base.ends_with("ch")
            || base.ends_with("sh")
        {
            format!("{}es", base)
        } else {
            format!("{}s", base)
        }
    }

    pub fn past_participle(base: &str) -> String {
        // Naive past participle generation
        if base.ends_with("e") {
            format!("{}d", base)
        } else {
            format!("{}ed", base)
        }
    }

    pub fn base(&self) -> &str {
        self.name
    }

    pub fn conjugated(&self) -> String {
        Self::conjugate(self.base())
    }

    pub fn past_participle_form(&self) -> String {
        Self::past_participle(self.base())
    }
}
