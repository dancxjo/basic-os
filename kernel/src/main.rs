#![no_std]
#![no_main]
#![feature(abi_x86_interrupt)]
#![feature(new_range_api)]

use kernel::Core;
use kernel_logger::init_logger;
use log::info;

extern crate alloc;

mod bootloader;
mod clock;
mod framebuffer;
mod gdt;
mod idt;
mod interrupts;
mod kernel;
mod kernel_logger;
mod log_entry;
mod memory;
mod mouse;
mod names;
mod panic;
mod proquints;
mod screen;
mod seed;
mod serial;

#[unsafe(no_mangle)]
pub extern "C" fn kmain() -> ! {
    init_logger();
    info!("Initializing kernel...");
    let mut k = Core::new();
    info!("Kernel initialized. Spin, spin, sugar...");
    k.spin();
}
