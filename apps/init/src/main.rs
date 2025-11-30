#![no_std]
#![no_main]

use userland::prelude::*;

#[no_mangle]
pub extern "C" fn _start() -> ! {
    userland::ensure_kernel_runtime();
    app_main()
}

#[panic_handler]
fn panic(info: &core::panic::PanicInfo) -> ! {
    userland::println!("Init Panic: {}", info);
    loop {}
}

fn app_main() -> ! {
    println!("Init process started.");

    // Spawn drivers
    println!("Spawning framebuffer_driver...");
    userland::sys::spawn("framebuffer_driver");

    println!("Spawning keyboard_driver...");
    userland::sys::spawn("keyboard_driver");

    println!("Spawning mouse_driver...");
    userland::sys::spawn("mouse_driver");

    // Spawn compositor
    println!("Spawning compositor...");
    userland::sys::spawn("compositor");

    // Spawn demo app
    println!("Spawning demo_app...");
    userland::sys::spawn("demo_app");

    println!("Init sequence complete. Entering idle loop.");
    loop {
        // TODO: Wait for children or handle signals
        // For now, just spin/sleep
        // We don't have a sleep syscall yet, so we'll just busy wait or yield if available
    }
}
