use alloc::boxed::Box;
use alloc::string::{String, ToString};
use alloc::sync::Arc;
use alloc::vec::Vec;
use core::fmt::Debug;
use serde::de::DeserializeOwned;
use spin::Mutex;
use uuid::Uuid;

// ----------------------------
// Basic Graph Types
// ----------------------------

#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub struct Fact {
    pub this: Uri,
    pub predicate: Predicate,
    pub that: Uri,
}

impl Fact {
    pub fn that(this: Uri, predicate: &Predicate, that: Uri) -> Fact {
        Fact {
            this,
            predicate: predicate.clone(),
            that,
        }
    }
}
// ----------------------------

#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub struct Uri(pub String);

#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub struct Predicate(pub &'static str);

#[macro_export]
macro_rules! does {
    ($lit:literal) => {
        &Predicate($lit)
    };
}

#[macro_export]
macro_rules! a_kind_of {
    ($lit:literal) => {
        $crate::Uri(concat!("kind:", $lit).into())
    };
}

// ----------------------------
// The Space Trait
// ----------------------------

pub trait Space: Send + Sync {
    fn protocols(&self) -> &[&'static str];

    fn content(&self, uri: &Uri) -> Option<&[u8]>;

    fn neighbors(&self, from: &Uri, pred: &Predicate) -> Vec<Uri>;

    fn kind(&self, _uri: &Uri) -> Option<Uri> {
        None
    }

    fn has_fact(&self, from: &Uri, pred: &Predicate, to: &Uri) -> bool {
        self.neighbors(from, pred).contains(to)
    }

    fn write_content(&mut self, _uri: &Uri, _data: &[u8]) -> Result<(), String> {
        Err("Read-only space".into())
    }

    fn add_fact(&mut self, _from: &Uri, _pred: Predicate, _to: &Uri) -> Result<(), String> {
        Err("Read-only space".into())
    }

    fn assert(&mut self, fact: Fact) -> Result<(), String> {
        self.add_fact(&fact.this, fact.predicate, &fact.that)
    }

    fn remove_fact(&mut self, _from: &Uri, _pred: &Predicate, _to: &Uri) -> Result<(), String> {
        Err("Read-only space".into())
    }

    fn facts(&self, from: &Uri) -> Vec<Fact> {
        let mut all = Vec::new();
        for pred in ["contains", "owns", "maps_to"].iter() {
            let p = Predicate(*pred);
            for tgt in self.neighbors(from, &p) {
                all.push(Fact::that(from.clone(), &p, tgt));
            }
        }
        all
    }
}

impl<T: Space + ?Sized> Space for &T {
    fn protocols(&self) -> &[&'static str] {
        (**self).protocols()
    }
    fn content(&self, uri: &Uri) -> Option<&[u8]> {
        (**self).content(uri)
    }
    fn neighbors(&self, from: &Uri, pred: &Predicate) -> Vec<Uri> {
        (**self).neighbors(from, pred)
    }
    fn kind(&self, uri: &Uri) -> Option<Uri> {
        (**self).kind(uri)
    }
    fn has_fact(&self, from: &Uri, pred: &Predicate, to: &Uri) -> bool {
        (**self).has_fact(from, pred, to)
    }
    fn write_content(&mut self, _uri: &Uri, _data: &[u8]) -> Result<(), String> {
        // Won't be called, but must exist
        Err("Cannot write through &T".into())
    }
    fn add_fact(&mut self, _from: &Uri, _pred: Predicate, _to: &Uri) -> Result<(), String> {
        Err("Cannot mutate through &T".into())
    }
    fn assert(&mut self, _fact: Fact) -> Result<(), String> {
        Err("Cannot mutate through &T".into())
    }
    fn remove_fact(&mut self, _from: &Uri, _pred: &Predicate, _to: &Uri) -> Result<(), String> {
        Err("Cannot mutate through &T".into())
    }
    fn facts(&self, from: &Uri) -> Vec<Fact> {
        (**self).facts(from)
    }
}

// ----------------------------
// Thing Proxy
// ----------------------------

pub struct Thing<'a> {
    pub uri: Uri,
    pub space: Arc<Mutex<dyn Space + 'a>>,
}

impl Thing<'_> {
    /// Convenience method for accessing this Thing's URI by value.
    pub fn uri(&self) -> Uri {
        self.uri.clone()
    }
    pub fn with<R>(&self, f: impl FnOnce(&dyn Space) -> R) -> R {
        let space = self.space.lock();
        f(&*space)
    }

    pub fn with_mut<R>(&self, f: impl FnOnce(&mut dyn Space) -> R) -> R {
        let mut space = self.space.lock();
        f(&mut *space)
    }

    pub fn neighbors(&self, pred: &Predicate) -> Vec<Uri> {
        self.with(|s| s.neighbors(&self.uri, pred))
    }

    pub fn content(&self) -> Option<Vec<u8>> {
        self.with(|s| s.content(&self.uri).map(|data| data.to_vec()))
    }

    pub fn kind(&self) -> Option<Uri> {
        self.with(|s| s.kind(&self.uri))
    }

    pub fn facts(&self) -> Vec<Fact> {
        self.with(|s| s.facts(&self.uri))
    }

    pub fn write(&self, data: &[u8]) -> Result<(), String> {
        self.with_mut(|s| s.write_content(&self.uri, data))
    }

    /// Cast to a Rust type if kind matches and deserialization succeeds.
    pub fn as_a<T: Thingable>(&self) -> Option<T> {
        let expected = T::kind_uri();
        let actual = self.kind()?;
        if actual != expected {
            return None;
        }
        let content = self.content()?;
        T::from_bytes(&content)
    }

    /// Get all related Things via a given predicate
    pub fn rel(&self, pred: &str) -> Vec<Thing> {
        let p = Predicate(Box::leak(pred.to_string().into_boxed_str()));
        let space = self.space.clone();
        self.neighbors(&p)
            .into_iter()
            .map(|uri| Thing {
                uri,
                space: space.clone(),
            })
            .collect()
    }

    pub fn related(&self, pred: &str) -> Vec<Thing> {
        self.rel(pred)
    }

    pub fn linked(&self, pred: &str) -> Vec<Thing> {
        self.rel(pred)
    }
}

// ----------------------------
// Thingable Trait
// ----------------------------

pub trait Thingable: Sized {
    fn kind_uri() -> Uri;
    fn from_bytes(data: &[u8]) -> Option<Self>;
}

impl<T> Thingable for T
where
    T: DeserializeOwned,
{
    fn kind_uri() -> Uri {
        Uri(concat!("kind:", stringify!(T)).into())
    }

    fn from_bytes(data: &[u8]) -> Option<Self> {
        postcard::from_bytes(data).ok()
    }
}

// ----------------------------
// Optional: ValidationError
// ----------------------------

#[derive(Debug)]
pub enum ValidationError {
    MissingKind,
    WrongKind { expected: Uri, actual: Uri },
    NoContent,
    BadFormat,
}

impl Thing<'_> {
    /// Validate cast attempt strictly: kind must match and deserialization must succeed.
    pub fn for_sure<T: Thingable>(&self) -> Result<T, ValidationError> {
        let expected = T::kind_uri();
        let actual = self.kind().ok_or_else(|| ValidationError::MissingKind)?;
        if actual != expected {
            return Err(ValidationError::WrongKind { expected, actual });
        }
        let content = self.content().ok_or_else(|| ValidationError::NoContent)?;
        T::from_bytes(&content).ok_or(ValidationError::BadFormat)
    }
}

pub fn make_uuid_from_seed(seed: &[u8]) -> Uuid {
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(seed);
    let hash = hasher.finalize();
    Uuid::from_bytes([
        hash[0], hash[1], hash[2], hash[3], hash[4], hash[5], hash[6], hash[7], hash[8], hash[9],
        hash[10], hash[11], hash[12], hash[13], hash[14], hash[15],
    ])
}
