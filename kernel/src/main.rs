#![no_std]
#![no_main]
#![feature(abi_x86_interrupt)]
#![feature(new_range_api)]

extern crate alloc;

mod bootloader;
mod fiat;
mod idt;
mod memory;
mod message;
mod panic;
mod seed;
mod serial;
mod thing;

use crate::alloc::borrow::ToOwned;
use limine::request::HhdmRequest;
use memory::BootFrameAllocator;
use message::Message;
use panic::halt;
use seed::SeedBlob;
use thing::Graph;

#[used]
static HHDM_REQUEST: HhdmRequest = HhdmRequest::new();

fn get_hhdm_offset() -> VirtAddr {
    let resp = HHDM_REQUEST.get_response().expect("No HHDM response");
    VirtAddr::new(resp.offset())
}

#[unsafe(no_mangle)]
pub extern "C" fn kmain() -> ! {
    memory::init_initial_allocator();
    let offset_to_lower_half = get_hhdm_offset();
    let mapper = unsafe { memory::init_paging(offset_to_lower_half) };
    // let frame_allocator = BootFrameAllocator::init();
    // memory::init_heap(mapper, frame_allocator);
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
    let hello_wasm =
        crate::bootloader::get_module("/boot/hello-user.wasm").expect("Failed to get module");
    let seed = SeedBlob {
        bytes: hello_wasm.to_vec(),
    };
    graph.insert("hello_seed", seed);
    if let Some(seed_thing) = graph.find_mut(|t| t.kind == "hello_seed") {
        let seed = seed_thing
            .data
            .as_typed::<SeedBlob>()
            .expect("Failed to get seed");
        serial_println!("Loaded WASM seed of length: {}", seed.bytes.len());
        let rv = run_wasm_thing(&seed.bytes);
        serial_println!("WASM thing returned: {}", rv);
    } else {
        serial_println!("No seed found");
    }

    graph.print_things();
    graph.print_links();

    loop {
        halt();
    }
}

use wasmi::{Caller, Func, FuncType, Linker, Store};
use wasmi::{Engine, Instance, Module, TypedFunc};
use x86_64::VirtAddr;

fn add_host_functions(linker: &mut Linker<()>) {
    linker
        .func_wrap(
            "env",
            "bang",
            |caller: Caller<'_, ()>, ptr: i32, len: i32| -> i64 {
                // call into your ThingOS kernel logic here
                serial_println!("bang called: ptr = {}, len = {}", ptr, len);
                42 // stub return
            },
        )
        .unwrap();

    linker
        .func_wrap(
            "env",
            "mutate",
            |caller: Caller<'_, ()>,
             id: i64,
             key_ptr: i32,
             key_len: i32,
             val_ptr: i32,
             val_len: i32| {
                serial_println!("mutate called on {}!", id);
            },
        )
        .unwrap();

    linker
        .func_wrap("env", "poof", |caller: Caller<'_, ()>, id: i64| {
            serial_println!("poof called on {}!", id);
        })
        .unwrap();
}

fn run_wasm_thing(bytes: &[u8]) -> i32 {
    let engine = Engine::default();
    let mut store = Store::new(&engine, ());
    let module = Module::new(&engine, bytes).expect("invalid module");

    let mut linker = Linker::new(&engine);
    add_host_functions(&mut linker); // add this!

    let instance = linker
        .instantiate(&mut store, &module)
        .expect("couldn't instantiate")
        .start(&mut store)
        .expect("start failed");
    42
}
