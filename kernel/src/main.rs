#![no_std]
#![no_main]

extern crate alloc;

mod memory;
mod message;
mod panic;
mod serial;
mod thing;

use crate::alloc::borrow::ToOwned;
use message::Message;
use panic::halt;
use thing::{Graph, Thing};

#[unsafe(no_mangle)]
pub extern "C" fn kmain() -> ! {
    memory::init_initial_allocator();
    let mut graph = Graph::new();
    let msg = Message {
        text: "ThingOS\nPeople, places, things and ideas\n© 2025".to_owned(),
    };
    graph.insert("message", msg);

    let echo_uuid;
    {
        let echo = graph
            .find_mut(|node| node.kind == "message")
            .expect("Failed to find boot message");

        let data = &mut echo.data;

        let as_typed = data.as_typed::<Message>().expect("Failed to get message");
        serial_println!("{}", as_typed.text);

        echo_uuid = echo.uuid.clone();
    }

    // as_typed.message = "Mutation".to_owned(); // Illegal as expected

    let and_another_thing = graph.get_mut(&echo_uuid).expect("Failed to get thing");
    let mutable_and_typed = and_another_thing
        .data
        .as_typed_mut::<Message>(echo_uuid.clone())
        .expect("Failed to get mutable message");

    mutable_and_typed.text = "Mutation".to_owned();

    serial_println!("Another thing: {:?}", and_another_thing);

    let immutable = graph.get(&echo_uuid).expect("Failed to get thing");
    let immutable_and_typed = immutable
        .data
        .as_typed::<Message>()
        .expect("Failed to get immutable message");

    serial_println!(
        "After mutation: {:?}, {}",
        immutable_and_typed,
        immutable_and_typed.text
    );

    graph.print_things();
    graph.print_links();

    loop {
        halt();
    }
}
