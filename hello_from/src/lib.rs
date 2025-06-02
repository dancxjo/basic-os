#![no_std]
#[macro_use]
mod prelude;
use crate::prelude::*;

#[unsafe(no_mangle)]
pub extern "C" fn main() {
    loop {
        println!("Hello from Task #{}", "xxxxxxx");
        for _ in 0..1_000_000_000 {
            // Busy wait to simulate work
        }
    }
}

#[panic_handler]
pub fn panic(_info: &core::panic::PanicInfo) -> ! {
    println!("\nPanic!");
    loop {}
}
