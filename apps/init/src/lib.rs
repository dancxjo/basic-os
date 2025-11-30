#![cfg_attr(not(feature = "std"), no_std)]
use userland::prelude::*;

pub fn app_main() -> ! {
    userland::println!("Init process started.");

    // Spawn drivers
    userland::println!("Spawning framebuffer_driver...");
    userland::sys::spawn("framebuffer_driver");

    userland::println!("Spawning keyboard_driver...");
    userland::sys::spawn("keyboard_driver");

    userland::println!("Spawning mouse_driver...");
    userland::sys::spawn("mouse_driver");

    // Spawn compositor
    userland::println!("Spawning compositor...");
    userland::sys::spawn("compositor");

    // Spawn demo app
    userland::println!("Spawning demo_app...");
    userland::sys::spawn("demo_app");

    // Spawn task list
    userland::println!("Spawning task_list...");
    userland::sys::spawn("task_list");

    // Spawn graph viewer
    userland::println!("Spawning graph_viewer...");
    userland::sys::spawn("graph_viewer");

    userland::println!("Init sequence complete. Entering idle loop.");
    loop {
        // TODO: Wait for children or handle signals
        // For now, just spin/sleep
        // We don't have a sleep syscall yet, so we'll just busy wait or yield if available
        // On host, we can sleep? No, this is no_std lib.
        // But userland might have sleep?
        // userland::sys::yield_now()?
    }
}
