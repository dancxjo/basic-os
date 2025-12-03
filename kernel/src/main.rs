#![no_std]
#![no_main]
#![feature(abi_x86_interrupt)]
#![feature(new_range_api)]

use logging::kernel_logger::init_logger;
use system::init_and_run_system;

extern crate alloc;

mod arch;
mod bootloader;
mod clock;
mod drivers;
mod graph;
mod logging;
mod mm;
mod system;
mod task;
pub mod trace;

#[cfg(all(feature = "kernel_multitask", feature = "single_process_desktop"))]
compile_error!("single_process_desktop cannot be combined with kernel_multitask");

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
        $crate::serial_println!($($arg)*);
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
