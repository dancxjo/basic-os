use alloc::{boxed::Box, vec::Vec};
use core::mem;

use crate::serial_println;

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

    pub fn as_mut_bytes(&mut self) -> Option<&mut [u8]> {
        match self {
            ThingData::Heap(b) => Some(b),
            ThingData::Typed(ptr, size) => unsafe {
                Some(core::slice::from_raw_parts_mut(*ptr as *mut u8, *size))
            },
            _ => None,
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

    pub fn as_typed_mut<T>(&mut self) -> Option<&mut T> {
        match self {
            ThingData::Typed(ptr, size) if *size == mem::size_of::<T>() => {
                Some(unsafe { &mut *(*ptr as *mut T) })
            }
            _ => None,
        }
    }
}

#[derive(Debug)]
pub struct Thing {
    pub kind: &'static str,
    pub name: &'static str,
    pub data: ThingData,
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
    pub things: Vec<Thing>,
    pub facts: Vec<Fact>,
    pub kinds: Vec<Kind>,
    pub predicates: Vec<Predicate>,
}

impl Graph {
    pub fn new() -> Self {
        Self {
            things: Vec::new(),
            facts: Vec::new(),
            kinds: Vec::new(),
            predicates: Vec::new(),
        }
    }

    pub fn insert<T: Thingable + 'static>(
        &mut self,
        key: &'static str,
        kind: &'static str,
        value: T,
    ) {
        let boxed = Box::leak(Box::new(value));
        let ptr = boxed as *mut T as *mut ();
        let size = mem::size_of::<T>();
        self.things.push(Thing {
            name: key,
            kind,
            data: ThingData::Typed(ptr, size),
        });
    }

    pub fn create_static_bytes(
        &mut self,
        name: &'static str,
        kind: &'static str,
        data: &'static [u8],
    ) -> usize {
        let thing = Thing {
            name,
            kind,
            data: ThingData::Bytes(data),
        };
        self.things.push(thing);
        self.things.len() - 1
    }

    pub fn create_heap_blob(
        &mut self,
        name: &'static str,
        kind: &'static str,
        data: Vec<u8>,
    ) -> usize {
        let thing = Thing {
            name,
            kind,
            data: ThingData::Heap(data.into_boxed_slice()),
        };
        self.things.push(thing);
        self.things.len() - 1
    }

    pub fn create_typed<T>(&mut self, name: &'static str, kind: &'static str, value: T) -> usize {
        let boxed = Box::leak(Box::new(value));
        let ptr = boxed as *mut T as *mut ();
        let size = mem::size_of::<T>();
        let thing = Thing {
            name,
            kind,
            data: ThingData::Typed(ptr, size),
        };
        self.things.push(thing);
        self.things.len() - 1
    }

    pub fn find_mut_by_name(&mut self, name: &str) -> Option<&mut Thing> {
        self.things.iter_mut().find(|t| t.name == name)
    }

    pub fn link(&mut self, this: usize, that: usize, predicate: &'static str) {
        let pred_idx = self
            .predicates
            .iter()
            .position(|p| p.name == predicate)
            .expect("Predicate must exist before linking!");
        self.facts.push(Fact {
            this,
            that,
            predicate: pred_idx,
        });
    }

    pub fn add_kind(&mut self, name: &'static str, description: &'static str) {
        self.kinds.push(Kind { name, description });
    }

    pub fn add_predicate(
        &mut self,
        name: &'static str,
        this_kind: &'static str,
        that_kind: &'static str,
    ) {
        self.predicates.push(Predicate {
            name,
            this_kind,
            that_kind,
        });
    }

    pub fn print_things(&self) {
        for (i, thing) in self.things.iter().enumerate() {
            serial_println!("#{}: {} [{}]", i, thing.name, thing.kind);
        }
    }

    pub fn print_links(&self) {
        for fact in &self.facts {
            let this = self.things[fact.this].name;
            let that = self.things[fact.that].name;
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

#[macro_export]
macro_rules! thing {
    (name: $name:expr, kind: $kind:expr, static: $data:expr) => {
        Thing {
            name: $name,
            kind: $kind,
            data: ThingData::Bytes($data),
        }
    };

    (name: $name:expr, kind: $kind:expr, value: $val:expr) => {{
        let leaked = Box::leak(Box::new($val));
        Thing {
            name: $name,
            kind: $kind,
            data: ThingData::Typed(leaked as *mut _ as *mut (), core::mem::size_of_val(leaked)),
        }
    }};
}
