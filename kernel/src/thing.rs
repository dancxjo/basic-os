use crate::serial_println;
use alloc::{
    boxed::Box,
    collections::{BTreeMap, BTreeSet},
    vec::Vec,
};
use core::any::Any;
use uuid::Uuid;

#[derive(Debug)]
pub enum ThingData {
    None,
    Bytes(&'static [u8]),
    Heap(Box<[u8]>),
    Owned(Box<dyn Any>),
}

impl ThingData {
    pub fn as_bytes(&self) -> Option<&[u8]> {
        match self {
            ThingData::Bytes(b) => Some(b),
            ThingData::Heap(b) => Some(b),
            _ => None,
        }
    }

    pub fn as_typed<T: 'static>(&self) -> Option<&T> {
        match self {
            ThingData::Owned(b) => b.downcast_ref::<T>(),
            _ => None,
        }
    }

    pub fn as_typed_mut<T: 'static>(&mut self, uuid: Uuid) -> Option<&mut T> {
        match self {
            ThingData::Owned(b) => {
                mark_dirty(uuid);
                b.downcast_mut::<T>()
            }
            _ => None,
        }
    }
}

#[derive(Debug)]
pub struct Thing {
    pub uuid: Uuid,
    pub kind: &'static str,
    pub data: ThingData,
}

impl Thing {
    pub fn new(kind: &'static str, data: ThingData) -> Self {
        let seed: &[u8] = match &data {
            ThingData::Bytes(b) => b,
            ThingData::Heap(b) => b,
            ThingData::Owned(_) => kind.as_bytes(),
            ThingData::None => &[],
        };
        let uuid = make_uuid_from_seed(seed);
        Self { uuid, kind, data }
    }
}

#[derive(Debug)]
pub struct Fact {
    pub this: usize,
    pub that: usize,
    pub predicate: usize,
}

#[derive(Debug)]
pub struct Kind {
    pub name: &'static str,
    pub description: &'static str,
}

#[derive(Debug)]
pub struct Predicate {
    pub name: &'static str,
    pub this_kind: &'static str,
    pub that_kind: &'static str,
}

pub struct Graph {
    pub uuid_map: BTreeMap<Uuid, usize>,
    pub things: Vec<Thing>,
    pub facts: Vec<Fact>,
    pub kinds: Vec<Kind>,
    pub predicates: Vec<Predicate>,
    pub dirty_set: BTreeSet<Uuid>,
}

impl Graph {
    pub fn new() -> Self {
        serial_println!("Establishing graph");
        Self {
            uuid_map: BTreeMap::new(),
            things: Vec::with_capacity(2),
            facts: Vec::new(),
            kinds: Vec::new(),
            predicates: Vec::new(),
            dirty_set: BTreeSet::new(),
        }
    }

    pub fn uuid_of<T: Thingable + 'static>(&self) -> Option<Uuid> {
        self.things
            .iter()
            .find_map(|thing| thing.data.as_typed::<T>().map(|_| thing.uuid))
    }

    pub fn insert(&mut self, _kind: &'static str, data: ThingData) -> Uuid {
        static mut COUNTER: u128 = 0xABCDEF1234567890;
        let uuid = unsafe {
            let value = COUNTER;
            COUNTER = COUNTER.wrapping_add(1);
            Uuid::from_u128(value)
        };

        let thing = Thing {
            uuid,
            kind: _kind,
            data,
        };

        let idx = self.things.len();
        self.uuid_map.insert(uuid, idx);
        self.things.push(thing);
        uuid
    }

    pub fn get<T: Thingable + 'static>(&self, uuid: &Uuid) -> Option<&T> {
        self.uuid_map
            .get(uuid)
            .and_then(|&i| self.things.get(i)?.data.as_typed::<T>())
    }

    pub fn get_mut<T: Thingable + 'static>(&mut self, uuid: &Uuid) -> Option<&mut T> {
        self.uuid_map
            .get(uuid)
            .copied()
            .and_then(move |i| self.things.get_mut(i)?.data.as_typed_mut::<T>(*uuid))
    }

    pub fn find<T: Thingable + 'static>(&self, f: impl Fn(&T) -> bool) -> Option<&T> {
        self.things
            .iter()
            .find_map(|thing| thing.data.as_typed::<T>().filter(|typed| f(*typed)))
    }

    pub fn find_mut<T: Thingable + 'static>(&mut self, f: impl Fn(&T) -> bool) -> Option<&mut T> {
        self.things.iter_mut().find_map(|thing| {
            thing
                .data
                .as_typed_mut::<T>(thing.uuid)
                .filter(|typed| f(*typed))
        })
    }

    pub fn find_one<T: Thingable + 'static>(&mut self) -> Option<&mut T> {
        self.find_mut::<T>(|_| true)
    }

    pub fn print_things(&self) {
        for (i, thing) in self.things.iter().enumerate() {
            serial_println!("#{}: {} [{}]", i, thing.uuid, thing.kind);
        }
    }

    pub fn print_links(&self) {
        for fact in &self.facts {
            let this = self.things[fact.this].uuid;
            let that = self.things[fact.that].uuid;
            let pred = self.predicates[fact.predicate].name;
            serial_println!("{} --{}--> {}", this, pred, that);
        }
    }
}

fn mark_dirty(uuid: Uuid) {
    serial_println!("Marked dirty: {}", uuid);
    // Future: queue this uuid or flag its memory page
}

pub trait Thingable: Sized {
    fn kind() -> &'static str;
    fn serialize(&self) -> Vec<u8>;
    fn deserialize(bytes: &[u8]) -> Option<Self>;
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

/// Convenience macro to wrap values in ThingData::Owned(Box::new(...))
#[macro_export]
macro_rules! thingify {
    ($value:expr) => {
        $crate::thing::ThingData::Owned(Box::new($value))
    };
}
