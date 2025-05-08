#![no_std]
#![no_main]
#![feature(abi_x86_interrupt)]
#![feature(new_range_api)]

use kernel_logger::init_logger;
use system::System;

extern crate alloc;

mod bootloader;
mod canon;
mod clock;
mod framebuffer;
mod gdt;
mod graph;
mod gui;
mod gui_output;
mod idt;
mod interrupts;
mod kernel_logger;
mod log_entry;
mod mouse;
mod panic;
mod screen;
mod tasks;
#[macro_use]
mod serial;
mod allocator;
mod input;
mod pic;
mod ps2;
mod stack;
mod system;

#[unsafe(no_mangle)]
pub extern "C" fn kmain() -> ! {
    init_logger();
    let mut os = System::new();
    os.run();
}

#[unsafe(no_mangle)]
pub extern "C" fn _start() -> ! {
    kmain();
}

#[macro_export]
macro_rules! println {
    ($($arg:tt)*) => {
        serial_println!($($arg)*);
    };
}
