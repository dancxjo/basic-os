use uuid::Uuid;

use crate::thing::Fact;

pub struct Pattern {
    pub subject: Option<Uuid>,
    pub verb: Option<Uuid>,
    pub object: Option<Uuid>,
}

impl Pattern {
    pub fn matches(&self, fact: &Fact) -> bool {
        (self.subject.is_none() || self.subject == Some(fact.subject))
            && (self.verb.is_none() || self.verb == Some(fact.verb))
            && (self.object.is_none() || self.object == Some(fact.object))
    }
}
