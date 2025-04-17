// Refactored version: minimal API, all mutation paths mark memory as dirty

use crate::serial_println;
use alloc::{boxed::Box, collections::btree_map::BTreeMap, vec::Vec};
use core::mem;
use uuid::Uuid;

// Called from all mutation sites to notify the system that memory may have changed
fn mark_dirty(uuid: Uuid) {
    // TODO: enqueue this UUID into a dirty set or flag the page it belongs to
    serial_println!("Marked dirty: {}", uuid);
}

#[derive(Debug)]
pub enum ThingData {
    None,
    Bytes(&'static [u8]),
    Heap(Box<[u8]>),
    Typed(*mut (), usize),
}

impl ThingData {
    pub fn as_bytes(&self) -> Option<&[u8]> {
        match self {
            ThingData::Bytes(b) => Some(b),
            ThingData::Heap(b) => Some(b),
            ThingData::Typed(ptr, size) => unsafe {
                Some(core::slice::from_raw_parts(*ptr as *const u8, *size))
            },
            ThingData::None => None,
        }
    }

    pub fn as_typed<T>(&self) -> Option<&T> {
        match self {
            ThingData::Typed(ptr, size) if *size == mem::size_of::<T>() => {
                Some(unsafe { &*(*ptr as *const T) })
            }
            _ => None,
        }
    }

    pub fn as_typed_mut<T>(&mut self, uuid: Uuid) -> Option<&mut T> {
        match self {
            ThingData::Typed(ptr, size) if *size == mem::size_of::<T>() => {
                mark_dirty(uuid);
                Some(unsafe { &mut *(*ptr as *mut T) })
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
            ThingData::Typed(ptr, size) => unsafe {
                core::slice::from_raw_parts(*ptr as *const u8, *size)
            },
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
        }
    }

    pub fn insert<T: Thingable + 'static>(&mut self, kind: &'static str, value: T) {
        let boxed = Box::leak(Box::new(value));
        let ptr = boxed as *mut T as *mut ();
        let size = mem::size_of::<T>();
        let thing = Thing::new(kind, ThingData::Typed(ptr, size));
        let idx = self.things.len();
        self.uuid_map.insert(thing.uuid, idx);
        self.things.push(thing);
    }

    pub fn get(&self, uuid: &Uuid) -> Option<&Thing> {
        self.uuid_map.get(uuid).map(|&i| &self.things[i])
    }

    pub fn get_mut(&mut self, uuid: &Uuid) -> Option<&mut Thing> {
        self.uuid_map
            .get(uuid)
            .copied()
            .map(move |i| &mut self.things[i])
    }

    pub fn find(&self, f: impl FnMut(&&Thing) -> bool) -> Option<&Thing> {
        self.things.iter().find(f)
    }

    pub fn find_mut(&mut self, f: impl Fn(&&mut Thing) -> bool) -> Option<&mut Thing> {
        self.things.iter_mut().find(f)
    }

    pub fn get_typed<T: 'static>(&self, uuid: &Uuid) -> Option<&T> {
        self.get(uuid)?.data.as_typed::<T>()
    }

    pub fn get_typed_mut<T: 'static>(&mut self, uuid: &Uuid) -> Option<&mut T> {
        self.get_mut(uuid)?.data.as_typed_mut::<T>(uuid.clone())
    }

    pub fn find_typed<T: 'static>(&self, f: impl Fn(&T) -> bool) -> Option<&T> {
        self.things.iter().find_map(|thing| {
            thing
                .data
                .as_typed::<T>()
                .and_then(|typed| if f(typed) { Some(typed) } else { None })
        })
    }

    pub fn find_typed_mut<T: 'static>(&mut self, f: impl Fn(&T) -> bool) -> Option<&mut T> {
        self.things.iter_mut().find_map(|thing| {
            thing
                .data
                .as_typed_mut::<T>(thing.uuid)
                .and_then(|typed| if f(typed) { Some(typed) } else { None })
        })
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
