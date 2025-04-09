// src/thing.rs
use alloc::string::String;
use alloc::vec::Vec;

#[derive(Debug)]
pub enum FieldBacking {
    Blob(&'static [u8]),
    Segment(*mut u8, usize),
    Synthesized,
}

#[derive(Debug)]
pub struct Thing<T> {
    pub kind: Option<&'static str>,
    pub name: Option<&'static str>,
    pub data: T,
    pub fields: Vec<(String, FieldBacking)>,
}
