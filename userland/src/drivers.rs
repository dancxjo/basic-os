//! Userland driver registry that announces hardware capabilities through the graph.
//! Add descriptors and edges here to keep driver state outside the kernel.

use crate::{canon, fiat, map, that, Value};
use alloc::string::ToString;
use uuid::Uuid;

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
}

impl DriverDescriptor {
    pub const fn new(name: &'static str, kind: DriverKind, description: &'static str) -> Self {
        Self {
            name,
            kind,
            description,
        }
    }

    pub fn id(&self) -> Uuid {
        driver_id(self.name)
    }
}

/// Built-in driver metadata. Extend this list to advertise new drivers
/// through the graph without touching the kernel.
pub const BUILTIN_DRIVERS: &[DriverDescriptor] = &[
    DriverDescriptor::new(
        "ps2-keyboard",
        DriverKind::Input,
        "PS/2 keyboard (set 1 scancodes)",
    ),
    DriverDescriptor::new(
        "ps2-mouse",
        DriverKind::Input,
        "PS/2 mouse (3-byte packets)",
    ),
    DriverDescriptor::new(
        "limine-framebuffer",
        DriverKind::Display,
        "Framebuffer provided by the Limine bootloader",
    ),
];

fn driver_kind_symbol(kind: DriverKind) -> crate::Symbol {
    match kind {
        DriverKind::Input => canon::DRIVER_INPUT,
        DriverKind::Display => canon::DRIVER_DISPLAY,
        DriverKind::Storage => canon::DRIVER_STORAGE,
        DriverKind::Timer => canon::DRIVER_TIMER,
        DriverKind::Other(_) => canon::DRIVER_OTHER,
    }
}

fn emit_driver_thing(driver: &DriverDescriptor, status: crate::Symbol) {
    let mut fields = map();
    fields.insert(canon::NAME, Value::text(driver.name));
    fields.insert(canon::STATUS, Value::symbol(status));
    fields.insert(canon::KIND, Value::symbol(driver_kind_symbol(driver.kind)));
    fields.insert(canon::TEXT, Value::text(driver.description.to_string()));

    fiat(Some(driver.id()), canon::DRIVER, fields);
}

/// Register the provided drivers as Things. Extend this helper with new
/// descriptors to keep driver metadata purely in userland.
pub fn register_drivers(drivers: &[DriverDescriptor]) {
    for driver in drivers {
        emit_driver_thing(driver, canon::INIT);
    }
}

pub fn register_builtin_drivers() {
    register_drivers(BUILTIN_DRIVERS);
}

/// Add a `STREAMS` edge from the named driver to a destination Thing.
pub fn connect_stream(name: &str, dst: Uuid, revision: u64) {
    that(driver_id(name), "STREAMS", dst, revision);
}

/// Stable UUID for a driver name (v5 namespace).
pub fn driver_id(name: &str) -> Uuid {
    crate::simple_uuid(name.as_bytes())
}
