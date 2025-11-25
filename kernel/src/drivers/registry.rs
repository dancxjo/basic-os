//! Declarative driver descriptors.
//!
//! A small, static registry meant to be easy for an LLM (or human) to extend.
//! New drivers only need to expose a `DriverDescriptor` with metadata and an
//! init function; everything else can discover them via `ALL_DRIVERS`.

use log::{info, warn};

pub type DriverInit = fn() -> Result<(), &'static str>;

#[derive(Clone, Copy)]
pub enum DriverKind {
    Input,
    Display,
    Storage,
    Timer,
    Other(&'static str),
}

#[derive(Clone, Copy)]
pub struct DriverDescriptor {
    pub name: &'static str,
    pub kind: DriverKind,
    pub description: &'static str,
    pub init: DriverInit,
}

impl DriverDescriptor {
    pub const fn new(
        name: &'static str,
        kind: DriverKind,
        description: &'static str,
        init: DriverInit,
    ) -> Self {
        Self {
            name,
            kind,
            description,
            init,
        }
    }
}

/// Registry of built-in drivers; new drivers can append to this list.
pub const ALL_DRIVERS: &[DriverDescriptor] = &[
    crate::drivers::keyboard::DRIVER,
    crate::drivers::mouse::DRIVER,
    crate::drivers::framebuffer::DRIVER,
];

pub fn init_all() {
    for driver in ALL_DRIVERS {
        match (driver.init)() {
            Ok(()) => info!("Driver '{}' initialized", driver.name),
            Err(err) => warn!("Driver '{}' failed to init: {}", driver.name, err),
        }
    }
}
