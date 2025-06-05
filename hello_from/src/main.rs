#![no_std]
#![no_main]

#[macro_use]
mod prelude;

#[unsafe(no_mangle)]
pub extern "C" fn _start() -> ! {
    loop {
        println!("Hello from Task #{}", "xxxxxxx");
        for _ in 0..1_000_000_000 {
            // Busy wait to simulate work
        }
    }
}

#[panic_handler]
pub fn panic(_info: &core::panic::PanicInfo) -> ! {
    println!("\nPanic on the streets of Birmingham!");
    loop {}
}
