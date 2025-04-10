#![no_std]
#![no_main]

extern crate alloc;

mod bootloader;
mod framebuffer;
mod graph;
mod helpers;
mod kernel;
mod keyboard;
mod keymaps;
mod log;
mod memory;
mod message_queue;
mod mouse;
mod panic;
mod serial;
mod stem;
mod textbox;
mod things;

#[unsafe(no_mangle)]
pub extern "C" fn kmain() -> ! {
    kernel::Kernel::init().run();
}
