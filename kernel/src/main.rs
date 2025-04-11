#![no_std]
#![no_main]

extern crate alloc;

mod memory;
mod message;
mod panic;
mod serial;
mod thing;

use message::Message;
use panic::halt;
use thing::Graph;

#[unsafe(no_mangle)]
pub extern "C" fn kmain() -> ! {
    memory::init_initial_allocator();
    let mut graph = Graph::new();
    let msg = Message {
        text: "ThingOS\nPeople, places, things and ideas\n© 2025",
    };
    graph.insert("boot_msg", "message", msg);

    let echo = graph
        .find_mut_by_name("boot_msg")
        .expect("Failed to find boot message");

    let as_typed = echo
        .data
        .as_typed::<Message>()
        .expect("Failed to get message");
    serial_println!("{}", as_typed.text);

    loop {
        halt();
    }
}
