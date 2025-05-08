use alloc::collections::BTreeMap;
use alloc::format;
use alloc::string::{String, ToString};
use alloc::sync::Arc;
use alloc::vec::Vec;
use alloc::{boxed::Box, fmt::Debug};
use alloc::{fmt, vec};
use erased_serde::Serialize as ErasedSerialize;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::canon::{self, FRAMEBUFFER1, IS_A, JOURNAL, KEYBOARD1, OWNS, SYSTEM};

// -------------------------------
// Trait: KindMeta
// -------------------------------
pub trait KindMeta: ErasedSerialize + Debug + Send + Sync + 'static + Clone + fmt::Display {
    fn static_uuid() -> Uuid {
        const NAMESPACE: Uuid = Uuid::NAMESPACE_OID;
        let name = Self::static_type_name();
        Uuid::new_v5(&NAMESPACE, name.as_bytes())
    }

    fn static_type_name() -> &'static str {
        core::any::type_name::<Self>()
    }
}

// -------------------------------
// Macro: kind_meta!
// -------------------------------
macro_rules! kind_meta {
    ($t:ty, $name:expr) => {
        impl KindMeta for $t {}

        impl fmt::Display for $t {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                write!(f, $name)
            }
        }
    };
}

// -------------------------------
// Trait: Kind
// -------------------------------
pub trait Kind: ErasedSerialize + Debug + Send + Sync + 'static + fmt::Display {
    fn uuid(&self) -> Uuid;
    fn type_name(&self) -> &'static str;
    fn clone_box(&self) -> Box<dyn Kind>;
    fn as_thing(&self) -> Thing<Self>
    where
        Self: Sized;

    fn as_anything(&self) -> Anything
    where
        Self: Sized + Clone,
    {
        let boxed: Box<dyn Kind> = Box::new(self.clone());
        Thing {
            id: self.uuid(),
            kind: canon::KIND_KIND,
            data: boxed,
            name: Some(self.type_name().into()),
        }
    }
}

erased_serde::serialize_trait_object!(Kind);

// Blanket impl for any KindMeta type:
impl<T> Kind for T
where
    T: KindMeta,
{
    fn uuid(&self) -> Uuid {
        T::static_uuid()
    }

    fn type_name(&self) -> &'static str {
        T::static_type_name()
    }

    fn clone_box(&self) -> Box<dyn Kind> {
        Box::new(self.clone())
    }

    fn as_thing(&self) -> Thing<Self>
    where
        Self: Sized,
    {
        Thing {
            id: self.uuid(),
            kind: canon::KIND_KIND,
            data: Box::new(self.clone()),
            name: Some(T::static_type_name().into()),
        }
    }
}

// -------------------------------
// Struct: Thing<T>
// -------------------------------
#[derive(Debug, Clone)]
pub struct Thing<T: ?Sized + Kind> {
    pub id: Uuid,
    pub kind: Uuid,           // UUID of the Thing<Kind>
    pub name: Option<String>, // optional, human-friendly
    pub data: Box<T>,
}

pub type Anything = Thing<dyn Kind>;

impl<T: Kind + 'static> Thing<T> {
    pub fn new(id: Uuid, data: T) -> Self {
        let kind_uuid = data.as_thing().id;
        Self {
            id,
            kind: kind_uuid,
            data: Box::new(data),
            name: Some("thing".into()),
        }
    }
}

impl<T: ?Sized + Kind> fmt::Display for Thing<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let id_str = uuid_base36(&self.id);

        if let Some(name) = &self.name {
            write!(f, "{} ({})", name, id_str)
        } else {
            // fallback: "a <typename>"
            write!(f, "a {} ({})", self.data.type_name(), id_str)
        }
    }
}

// -------------------------------
// Struct: Fact
// -------------------------------
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Fact {
    pub subject: Uuid,
    pub verb: Uuid,
    pub object: Uuid,
}

// -------------------------------
// Struct: Transaction
// -------------------------------
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Transaction {
    pub timestamp: u128,
    pub facts: Vec<Fact>,
}

// -------------------------------
// Bootstrap Types
// -------------------------------
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KindKind;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KernelKind;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JournalKind;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceKind;

kind_meta!(KindKind, "Kind");
kind_meta!(KernelKind, "Kernel");
kind_meta!(JournalKind, "Journal");
kind_meta!(DeviceKind, "Device");

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerbKind;

kind_meta!(VerbKind, "Verb");

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IsAKind;

kind_meta!(IsAKind, "is_a");

// -------------------------------
// Description: Derived Matchers
// -------------------------------
#[derive(Debug, Clone)]
pub enum Description {
    IsA(Uuid),
    And(Vec<Description>),
    Not(Box<Description>),
    Implies {
        head: Box<Description>,
        body: Box<Description>,
    },
}

// -------------------------------
// Struct: Graph
// -------------------------------
pub struct Graph {
    pub things: BTreeMap<Uuid, Arc<Anything>>,
    pub facts: Vec<Fact>,
}

impl Graph {
    pub fn new() -> Self {
        Self {
            things: BTreeMap::new(),
            facts: vec![],
        }
    }

    pub fn add_thing(&mut self, thing: Anything) {
        self.things.insert(thing.id, Arc::new(thing));
    }

    pub fn add_fact(&mut self, fact: Fact) {
        let s = self
            .get_thing(&fact.subject)
            .map(|t| format!("{}", t))
            .unwrap_or_else(|| format!("{} ({})", "something", uuid_base36(&fact.subject)));

        let v = self
            .get_thing(&fact.verb)
            .map(|t| format!("{}", t))
            .unwrap_or_else(|| format!("{} ({})", "does something to", uuid_base36(&fact.verb)));

        let o = self
            .get_thing(&fact.object)
            .map(|t| format!("{}", t))
            .unwrap_or_else(|| format!("{} ({})", "something", uuid_base36(&fact.object)));

        log::info!("{} {} {}", s, v, o);
        self.facts.push(fact);
    }

    pub fn get_thing(&self, id: &Uuid) -> Option<Arc<Anything>> {
        self.things.get(id).cloned()
    }

    pub fn is_a(&self, x: &Uuid, y: &Uuid) -> bool {
        self.things.get(x).map(|t| t.kind == *y).unwrap_or(false)
    }

    pub fn matches(&self, thing: &Anything, desc: &Description) -> bool {
        match desc {
            Description::IsA(kind_id) => thing.kind == *kind_id,
            Description::And(clauses) => clauses.iter().all(|d| self.matches(thing, d)),
            Description::Not(inner) => !self.matches(thing, inner),
            Description::Implies { head, body } => {
                if self.matches(thing, body) {
                    self.matches(thing, head)
                } else {
                    true
                }
            }
        }
    }

    pub fn query<'a>(&'a self, desc: &'a Description) -> impl Iterator<Item = &'a Arc<Anything>> {
        self.things
            .values()
            .filter(move |thing| self.matches(thing, desc))
    }

    pub fn derived_facts(&self) -> impl Iterator<Item = Fact> + '_ {
        self.things.values().map(|t| Fact {
            subject: t.id,
            verb: canon::IS_A,
            object: t.kind,
        })
    }
}

// -------------------------------
// Registration Example
// -------------------------------
pub fn bootstrap_graph() -> Graph {
    let mut graph = Graph::new();

    // Add KindKind
    let kind_kind = KindKind;
    graph.add_thing(kind_kind.as_anything());

    // Add Kernel
    let kernel_kind = KernelKind;
    graph.add_thing(kernel_kind.as_anything());

    // Add Journal
    let journal_kind = JournalKind;
    graph.add_thing(journal_kind.as_anything());

    // Add Keyboard (DeviceKind)
    let keyboard_kind = DeviceKind;
    graph.add_thing(keyboard_kind.as_anything());

    // Add Framebuffer (DeviceKind)
    let framebuffer_kind = DeviceKind;
    graph.add_thing(framebuffer_kind.as_anything());

    // Add initial Facts
    let verb_kind = VerbKind;
    graph.add_thing(verb_kind.as_anything());

    let is_a = IsAKind;
    graph.add_thing(is_a.as_anything());

    graph.add_fact(Fact {
        subject: SYSTEM,
        verb: IS_A,
        object: canon::KERNEL_KIND,
    });

    graph.add_fact(Fact {
        subject: KEYBOARD1,
        verb: IS_A,
        object: DeviceKind::static_uuid(),
    });

    graph.add_fact(Fact {
        subject: FRAMEBUFFER1,
        verb: IS_A,
        object: DeviceKind::static_uuid(),
    });

    graph.add_fact(Fact {
        subject: SYSTEM,
        verb: OWNS,
        object: JOURNAL,
    });

    graph
}

fn uuid_base36(uuid: &Uuid) -> String {
    let num = uuid.as_u128();
    let encoded = base_x::encode("0123456789abcdefghijklmnopqrstuvwxyz", &num.to_be_bytes());
    let trimmed = encoded.trim_start_matches('0');
    trimmed.to_string()
}
