use core::any::Any;
use core::arch::asm;

use crate::interrupts::init_interrupts;
use crate::memory::MEMMAP_REQUEST;
use crate::mouse::Mouse;

use crate::thing::Thing;
use crate::thingify;
use crate::{
    bootloader, dump_overlay,
    framebuffer::Framebuffer,
    gdt::init_gdt,
    idt::{init_double_fault_stack, init_idt},
    memory::{self, HEAP_SIZE, HEAP_START, map_page_to},
    message::Message,
    seed::SeedBlob,
    serial_println,
    thing::{Graph, ThingData, Thingable},
};
use alloc::{boxed::Box, format, vec::Vec};
use limine::request::HhdmRequest;
use x86_64::{
    VirtAddr,
    structures::paging::{FrameAllocator, Mapper, Page, PageTableFlags as Flags},
};

pub const USER_BASE_VADDR: u64 = 0x4000_0000;

pub struct Kernel {
    pub graph: Graph,
    mapper: &'static mut x86_64::structures::paging::OffsetPageTable<'static>,
    frame_allocator: memory::BootFrameAllocator,
    framebuffer: Framebuffer,
}

#[used]
pub static HHDM_REQUEST: HhdmRequest = HhdmRequest::new();

fn get_hhdm_offset() -> VirtAddr {
    let resp = HHDM_REQUEST.get_response().expect("No HHDM response");
    VirtAddr::new(resp.offset())
}

impl Kernel {
    pub fn spin(&mut self) -> ! {
        let mut x: usize = 0;
        let mut direction: isize = 1;
        let mouse_id = self.graph.uuid_of::<Mouse>().unwrap();
        let mut tick: u64 = 0;

        loop {
            // self.framebuffer.clear(0x202020);

            // Draw bouncing block
            self.framebuffer.draw_rect(x, 100, 100, 50, 0x00FF00);

            // Dynamic text
            let msg = format!("tick: {}", tick);
            self.framebuffer.draw_text(&msg, x, 200, 20);
            self.framebuffer.draw_text("ThingOS Kernel UI", 20, 20, 24);
            self.framebuffer
                .draw_text("Dragons not yet implemented", 20, 50, 16);

            // Draw mouse pointer
            let mouse: &mut Mouse = self.graph.get_mut::<Mouse>(&mouse_id).unwrap();
            mouse.poll();
            serial_println!("Mouse position: ({}, {})", mouse.x, mouse.y);
            mouse.draw(&mut self.framebuffer);

            self.framebuffer.flush();

            x = (x as isize + direction) as usize;
            if x > self.framebuffer.width - 100 || x == 0 {
                direction *= -1;
            }

            tick = tick.wrapping_add(1);
        }
    }

    pub fn new() -> Self {
        let offset = get_hhdm_offset();
        let (mut mapper, mut frame_allocator) = unsafe { memory::init(offset) };
        serial_println!("Paging initialized");

        init_gdt();
        #[allow(static_mut_refs)]
        let tss = unsafe { crate::gdt::TSS.as_mut().expect("TSS not initialized") };
        init_double_fault_stack(tss, &mut mapper, &mut frame_allocator);
        init_idt();
        init_interrupts();
        x86_64::instructions::interrupts::enable();

        let graph = Graph::new();
        let framebuffer = Framebuffer::new().expect("Framebuffer not initialized");
        let mouse = Mouse::new(&framebuffer);
        let boxed = Box::new(42_u64);
        let addr = boxed.as_ref() as *const u64 as usize;

        serial_println!("Boxed value address: {:#x}", addr);
        assert!(
            addr >= HEAP_START as usize && addr < (HEAP_START + HEAP_SIZE as u64) as usize,
            "Boxed value not in heap!"
        );

        let mut kernel = Kernel {
            graph,
            mapper,
            frame_allocator,
            framebuffer,
        };

        kernel.init_graph();
        kernel.print();
        kernel.map_bootloader_memory();
        kernel.print();
        kernel.insert_devices(mouse);
        kernel.print();

        kernel
    }

    fn init_graph(&mut self) {
        self.graph.insert(
            "message",
            thingify!(Message::new("ThingOS. People, places, things and ideas.")),
        );

        self.graph.insert("kernel.version", thingify!("v0.1.0"));
        self.graph
            .insert("kernel.build_id", thingify!(0xDEADBEEF_u64));
    }

    fn map_bootloader_memory(&mut self) {
        let hhdm_response = HHDM_REQUEST.get_response().expect("No HHDM response");
        let memory_map = MEMMAP_REQUEST.get_response().expect("No memory map");

        let base = hhdm_response.offset();
        let max_phys = memory_map
            .entries()
            .iter()
            .map(|e| e.base + e.length)
            .max()
            .unwrap_or(0);

        let len = max_phys;

        let slice = unsafe { core::slice::from_raw_parts(base as *const u8, len as usize) };
        self.graph.insert("allocated memory", thingify!(slice));
    }

    fn insert_devices(&mut self, mouse: Mouse) {
        self.graph.insert("mouse", thingify!(mouse));
        self.graph.insert("cursor.color", thingify!(0xFF0000_u32));
    }

    fn print(&self) {
        self.graph.print_things();
        dump_overlay!(&self.graph);
    }

    fn get_elf_entry_point(&mut self) -> u64 {
        let this = self
            .graph
            .find_one::<SeedBlob>()
            .expect("Missing seed_blob");
        let data = &this.bytes;
        if &data[0..4] != b"\x7FELF" {
            panic!("Invalid ELF magic");
        }
        u64::from_le_bytes(data[24..32].try_into().unwrap()) + USER_BASE_VADDR
    }

    pub fn enrich_graph(&mut self) {
        serial_println!("[kernel] Enriched graph with bootloader memory map");
        self.graph.print_things();
    }
}
