#![no_std]
#![no_main]

extern crate alloc;

mod framebuffer;
mod graph;
mod helpers;
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
mod thing;

use helpers::thingify::{
    collect_memory_regions, thingify_boot_memory, thingify_kernel, thingify_ui_layout,
};

use alloc::{boxed::Box, vec};
use framebuffer::Framebuffer;
use graph::Graph;
use keyboard::Keyboard;
use keymaps::US_ALTGR_INTL;
use log::LOG_MESSAGES;
use mouse::Mouse;
use stem::update_pointer_position;
use textbox::TextBox;

unsafe extern "C" {
    static _start: u8;
    static _etext: u8;
    static _edata: u8;
    static _end: u8;
}

#[unsafe(no_mangle)]
pub extern "C" fn kmain() -> ! {
    memory::init_heap();
    println!("Heap initialized");
    let mut fb = Framebuffer::init().expect("No framebuffer found");
    let mut keyboard = Keyboard::init();
    let mut mouse = Mouse::init();
    let mut buffer = TextBox::new(48, 48, fb.width() - 48, fb.height() - 48);
    let mut graph = Graph {
        things: vec![],
        facts: vec![],
        kinds: vec![],
        predicates: vec![],
    };
    println!("Graph initialized");
    thingify_kernel(&mut graph);
    println!("Kernel thingified");
    let regions = collect_memory_regions();
    thingify_boot_memory(&mut graph, regions);
    println!("Memory regions thingified");

    thingify_ui_layout(&mut graph);
    println!("UI layout thingified");

    for thing in &graph.things {
        println!("Thing: {:?} ({:?})", thing.name, thing.kind);
    }

    let mut dirty = true;
    fb.clear(0x000000); // Black background
    // fb.draw_text(32, 32, "Hello! This is the new Thing OS!", 0xFFFFFF);
    // fb.flush();

    let mut keyboard_buffer = vec![];

    loop {
        // Drain messages from the log queue
        {
            let mut messages = LOG_MESSAGES.lock();
            let mut drained_messages = vec![];
            while let Some(msg) = messages.pop() {
                drained_messages.push(msg);
            }
            drop(messages); // Explicitly drop the lock

            for msg in drained_messages {
                for line in msg.lines() {
                    buffer.insert_string(line);
                    buffer.insert_char('\n');
                }
                dirty = true
            }
        }

        // Somewhere in main loop after processing logs:
        if let Some(log_thing) = graph
            .things
            .iter_mut()
            .find(|t| t.name == Some("window.log"))
        {
            let mut combined = alloc::string::String::new();
            for msg in &buffer.content {
                combined.push(*msg);
            }
            log_thing.data = Box::leak(combined.into_boxed_str()).as_bytes();
        }

        if let Some(key) = keyboard.poll_key() {
            keyboard_buffer.push(key);
            if let Some(ch) = US_ALTGR_INTL[key as usize].normal {
                buffer.insert_char(ch);
                dirty = true;
            }
        }

        if let Some((dx, dy, buttons)) = mouse.poll() {
            update_pointer_position(&mut graph, &fb, dx, dy);
            dirty = true;
        }
        // buffer.draw(&mut fb);

        if dirty {
            stem::draw_stem_ui(&graph, &mut fb);
            fb.flush();
        }
    }
}
