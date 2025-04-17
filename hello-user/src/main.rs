#![no_std]
#![no_main]

use core::panic::PanicInfo;

#[panic_handler]
fn panic(_: &PanicInfo) -> ! {
    loop {}
}

#[unsafe(no_mangle)]
pub extern "C" fn _start() -> ! {
    // For now, just loop forever to prove this code runs.
    loop {
        // You could write to a memory-mapped address if you had one.
    }
}
