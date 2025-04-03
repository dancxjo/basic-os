#![no_std]
#![no_main]

extern crate alloc;

mod framebuffer;
mod heap;
mod input;
mod keymaps;
mod panic;
mod serial;
mod textbox;
mod textregion;
mod ui;

use framebuffer::Framebuffer;
use input::Keyboard;
use textregion::TextRegion;

#[unsafe(no_mangle)]
pub extern "C" fn kmain() -> ! {
    heap::init_heap();

    // Initialize core subsystems
    let mut fb = Framebuffer::init().expect("No framebuffer found");
    let mut keyboard = Keyboard::init();

    let mut buffer = TextRegion::new(0, 0, fb.width(), fb.height());

    // Draw initial screen
    fb.clear(0x000000); // Black background

    loop {
        if let Some(key) = keyboard.poll_key() {
            if let Some(_ch) = buffer.insert_key(key, &keyboard.modifiers) {
                buffer.draw(&mut fb);
            }
        }

        fb.flush();
    }
}
