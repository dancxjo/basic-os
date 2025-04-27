#![no_std]
#![no_main]
#![feature(abi_x86_interrupt)]
#![feature(new_range_api)]

use kernel_logger::init_logger;
use log::info;
use thingos::ThingOS;

extern crate alloc;

mod beat;
mod bootloader;
mod clock;
mod effects;
mod framebuffer;
mod gdt;
mod gui;
mod gui_output;
mod idt;
mod interrupts;
mod kernel_logger;
mod log_entry;
mod memory;
mod mouse;
mod names;
mod panic;
mod pattern;
mod proquints;
mod screen;
mod serial;
mod space;
mod thing;
mod thingos;
mod verb;

#[unsafe(no_mangle)]
pub extern "C" fn kmain() -> ! {
    init_logger();
    info!("Initializing ThingOS...");
    let mut os = ThingOS::new();
    info!("Running ThingOS...");
    os.run();
}
