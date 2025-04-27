extern crate alloc;
use alloc::{boxed::Box, vec::Vec};
use erased_serde::serialize_trait_object;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

serialize_trait_object!(Kind);

// A veridically neutral triple stored in the journal
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Fact {
    pub subject: Uuid,
    pub verb: Uuid,
    pub object: Uuid,
    pub negated: bool,
    pub timestamp: u64,
}

impl Fact {
    pub fn new(subject: Uuid, verb: Uuid, object: Uuid, negated: bool) -> Self {
        let mut namespace_data = Vec::new();
        namespace_data.extend_from_slice(subject.as_bytes());
        namespace_data.extend_from_slice(verb.as_bytes());
        namespace_data.extend_from_slice(object.as_bytes());
        let id = Uuid::new_v5(&Uuid::NAMESPACE_OID, &namespace_data);
        Fact {
            subject,
            verb,
            object,
            negated,
            timestamp: 0,
        }
    }

    pub fn matches(&self, other: &Fact) -> bool {
        self.subject == other.subject
            && self.verb == other.verb
            && self.object == other.object
            && self.negated == other.negated
    }

    pub fn retract(subject: Uuid, verb: Uuid, object: Uuid) -> Self {
        Self {
            subject,
            verb,
            object,
            negated: true,
            timestamp: 0,
        }
    }
}

pub trait Kind: erased_serde::Serialize {
    fn type_name() -> &'static str
    where
        Self: Sized;
    fn uuid() -> Uuid
    where
        Self: Sized;

    fn as_serialize(&self) -> &dyn erased_serde::Serialize;
    fn clone_box(&self) -> Box<dyn Kind>;
    fn as_any(&self) -> &dyn core::any::Any;
    fn as_any_mut(&mut self) -> &mut dyn core::any::Any;

    fn type_name_dyn(&self) -> &'static str;
}

impl<T> Kind for T
where
    T: erased_serde::Serialize + Clone + 'static,
{
    fn type_name() -> &'static str {
        core::any::type_name::<T>()
    }

    fn type_name_dyn(&self) -> &'static str {
        Self::type_name()
    }

    fn uuid() -> Uuid {
        Uuid::new_v5(&Uuid::NAMESPACE_OID, Self::type_name().as_bytes())
    }

    fn as_serialize(&self) -> &dyn erased_serde::Serialize {
        self
    }

    fn clone_box(&self) -> Box<dyn Kind> {
        Box::new(self.clone())
    }

    fn as_any(&self) -> &dyn core::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn core::any::Any {
        self
    }
}

// A uniquely identified typed instance
pub struct Thing {
    pub id: Uuid,
    pub kind: Uuid,
    pub data: Box<dyn Kind>,
}

impl Thing {
    pub fn new<T: Kind + serde::Serialize + 'static>(data: T) -> Self {
        Self {
            id: Uuid::new_v5(
                &Uuid::NAMESPACE_OID,
                postcard::to_allocvec(&data).unwrap_or_default().as_slice(),
            ),
            kind: T::uuid(),
            data: Box::new(data),
        }
    }

    pub fn diff(&self, other: &Self) -> Option<Fact> {
        let a = postcard::to_allocvec(&*self.data).ok()?;
        let b = postcard::to_allocvec(&*other.data).ok()?;
        if a != b {
            Some(Fact::new(
                self.id,
                Uuid::new_v5(&Uuid::NAMESPACE_OID, b"has_changed"),
                Uuid::new_v5(&Uuid::NAMESPACE_OID, &a),
                false,
            ))
        } else {
            None
        }
    }
}

use core::fmt;

impl fmt::Display for Thing {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.id,)
    }
}

pub struct Query<'a, T> {
    pub iter: Box<dyn Iterator<Item = &'a T> + 'a>,
}

impl<'a, T> Query<'a, T> {
    pub fn one(self) -> Option<&'a T> {
        self.iter.into_iter().next()
    }

    pub fn all(self) -> Vec<&'a T> {
        self.iter.collect()
    }
}
