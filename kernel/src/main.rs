#![no_std]
#![no_main]
#![feature(abi_x86_interrupt)]
#![feature(new_range_api)]

use kernel_logger::init_logger;
use system::{System, init_and_run_system};

extern crate alloc;

mod allocator;
mod bootloader;
mod canon;
mod clock;
mod compositor;
mod framebuffer;
mod gdt;
mod graph;
mod gui;
mod gui_output;
mod hello_task;
mod idt;
mod input;
mod interrupts;
mod kernel_logger;
mod log_entry;
mod mouse;
mod panic;
mod pic;
mod ps2;
mod scheduler;
mod screen;
mod serial;
mod stack;
mod system;

#[unsafe(no_mangle)]
pub extern "C" fn kmain() -> ! {
    init_logger();
    init_and_run_system();
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

#[macro_export]
macro_rules! bootstrap_step {
    ($desc:expr, $block:expr) => {{
        info!("Initializing {}...", $desc);
        let result = $block;
        info!("Init {} complete.\n", $desc);
        result
    }};
}
