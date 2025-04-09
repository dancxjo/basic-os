//! main.rs - Kernel Entry Point
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

use alloc::{string::String, vec, vec::Vec};
use framebuffer::Framebuffer;
use graph::{Graph, ThingData};
use helpers::thingify::{
    collect_memory_regions, thingify_boot_memory, thingify_kernel, thingify_ui_layout,
};
use keyboard::Keyboard;
use keymaps::US_ALTGR_INTL;
use log::LOG_MESSAGES;
use mouse::Mouse;
use stem::{draw_stem_ui, update_pointer_position};
use textbox::TextBox;

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
        println!("Thing: {} [{}]", thing.name, thing.kind);
    }

    let mut dirty = true;
    fb.clear(0x000000); // Black background
    let mut keyboard_buffer = vec![];

    loop {
        // Drain messages from log queue
        let drained_messages = {
            let mut messages = LOG_MESSAGES.lock();
            let mut out = Vec::new();
            while let Some(msg) = messages.pop() {
                out.push(msg);
            }
            out
        };

        for msg in drained_messages {
            for line in msg.lines() {
                buffer.insert_string(line);
                buffer.insert_char('\n');
            }
            dirty = true;
        }

        if let Some(log_thing) = graph.find_mut_by_name("window.log") {
            let mut combined = String::new();
            for msg in &buffer.content {
                combined.push(*msg);
            }
            let blob = combined.into_bytes().into_boxed_slice();
            log_thing.data = ThingData::Heap(blob);
        }

        if let Some(key) = keyboard.poll_key() {
            keyboard_buffer.push(key);
            if let Some(ch) = US_ALTGR_INTL[key as usize].normal {
                buffer.insert_char(ch);
                dirty = true;
            }
        }

        if let Some((dx, dy, _buttons)) = mouse.poll() {
            update_pointer_position(&mut graph, &fb, dx, dy);
            // dirty = true;
        }

        if dirty {
            draw_stem_ui(&graph, &mut fb);
            fb.flush();
        }
    }
}
