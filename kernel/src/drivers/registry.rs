//! Declarative driver descriptors.
//!
//! A small, static registry meant to be easy for an LLM (or human) to extend.
//! New drivers only need to expose a `DriverDescriptor` with metadata and an
//! init function; everything else can discover them via `ALL_DRIVERS`.

use crate::telemetry::canon::{self, Symbol};
use crate::telemetry::journal::{self, Event, Value};
use alloc::collections::BTreeMap;
use alloc::string::String;
use log::{info, warn};
use uuid::Uuid;

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

fn driver_kind_symbol(kind: DriverKind) -> Symbol {
    match kind {
        DriverKind::Input => canon::DRIVER_INPUT,
        DriverKind::Display => canon::DRIVER_DISPLAY,
        DriverKind::Storage => canon::DRIVER_STORAGE,
        DriverKind::Timer => canon::DRIVER_TIMER,
        DriverKind::Other(_) => canon::DRIVER_OTHER,
    }
}

fn emit_driver_thing(driver: &DriverDescriptor, status: Symbol) {
    let id = Uuid::new_v5(&Uuid::NAMESPACE_OID, driver.name.as_bytes());

    let mut fields = BTreeMap::new();
    fields.insert(canon::NAME, Value::Text(String::from(driver.name)));
    fields.insert(canon::STATUS, Value::Symbol(status));
    fields.insert(canon::KIND, Value::Symbol(driver_kind_symbol(driver.kind)));

    let mut data = BTreeMap::new();
    data.insert(canon::ID, Value::Uuid(id));
    data.insert(canon::KIND, Value::Symbol(canon::DRIVER));
    data.insert(canon::REVISION, Value::U64(0));
    data.insert(canon::FIELDS, Value::Map(fields));

    let event = Event::new(canon::THING_CREATED, Value::Map(data));
    let _ = journal::emit(event);
}

pub fn init_all() {
    for driver in ALL_DRIVERS {
        match (driver.init)() {
            Ok(()) => {
                info!("Driver '{}' initialized", driver.name);
                emit_driver_thing(driver, canon::INIT);
            }
            Err(err) => {
                warn!("Driver '{}' failed to init: {}", driver.name, err);
                emit_driver_thing(driver, canon::FAIL);
            }
        }
    }
}
