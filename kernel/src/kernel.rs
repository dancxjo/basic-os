use crate::{
    dump_overlay,
    memory::{self, HEAP_SIZE, HEAP_START},
    message::Message,
    serial_println,
    thing::Graph,
};
use alloc::{boxed::Box, format};
use limine::memory_map::Entry;
use limine::request::HhdmRequest;
use x86_64::VirtAddr;

pub struct Kernel {
    pub graph: Graph,
    // future: scheduler, indexers, caches, etc.
}

#[used]
static HHDM_REQUEST: HhdmRequest = HhdmRequest::new();

fn get_hhdm_offset() -> VirtAddr {
    let resp = HHDM_REQUEST.get_response().expect("No HHDM response");
    VirtAddr::new(resp.offset())
}

impl Kernel {
    pub fn new() -> Self {
        let offset = get_hhdm_offset();
        let (mapper, mut allocator) = unsafe { memory::init(offset) };

        serial_println!("Paging initialized");

        let boxed = Box::new(42_u64);
        let addr = boxed.as_ref() as *const u64 as usize;
        serial_println!("Boxed value address: {:#x}", addr);
        assert!(
            addr >= HEAP_START as usize && addr < (HEAP_START + HEAP_SIZE as u64) as usize,
            "Boxed value not in heap!"
        );

        let mut graph = Graph::new();
        dump_overlay!(&graph);

        let message = Message::new("ThingOS. People, places, things and ideas.");
        graph.insert("message", message);
        dump_overlay!(&graph);

        Kernel { graph }
    }

    pub fn spin(&mut self) -> ! {
        for i in 0.. {
            let m = Message::new(format!("ThingOS: {}", i).as_str());
            self.graph.insert("message", m);
            if i % 1000 == 0 {
                serial_println!("Boxed {}", i);
                dump_overlay!(&self.graph);
            }
        }
        loop {
            crate::panic::halt();
        }
    }
}
