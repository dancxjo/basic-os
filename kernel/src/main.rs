#![no_std]
#![no_main]
#![feature(abi_x86_interrupt)]
#![feature(new_range_api)]

use kernel_logger::init_logger;
use thingos::ThingOS;

extern crate alloc;

mod bootloader;
mod clock;
mod framebuffer;
mod gdt;
mod gui;
mod gui_output;
mod idt;
mod interrupts;
mod kernel_logger;
mod kthread;
mod log_entry;
mod mouse;
mod panic;
mod screen;
#[macro_use]
mod serial;
mod allocator;
mod paging;
mod stack;
mod thingos;

#[unsafe(no_mangle)]
pub extern "C" fn kmain() -> ! {
    init_logger();
    let mut os = ThingOS::new();
    os.run();
}

#[macro_export]
macro_rules! println {
    ($($arg:tt)*) => {
        serial_println!($($arg)*);
    };
}
