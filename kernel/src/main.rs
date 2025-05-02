#![no_std]
#![no_main]
#![feature(abi_x86_interrupt)]
#![feature(new_range_api)]

use core::arch::asm;

use kernel_logger::init_logger;
use os::OS;

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
mod log_entry;
mod mouse;
mod panic;
mod screen;
#[macro_use]
mod serial;
mod allocator;
mod os;
mod pic;
mod stack;
mod tasks;

#[unsafe(no_mangle)]
pub extern "C" fn kmain() -> ! {
    init_logger();
    let mut os = OS::new();
    os.run();
}

#[unsafe(no_mangle)]
pub extern "C" fn _start() -> ! {
    // unsafe { init_stack() };
    kmain();
}

#[macro_export]
macro_rules! println {
    ($($arg:tt)*) => {
        serial_println!($($arg)*);
    };
}
