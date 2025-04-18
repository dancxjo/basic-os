// Kernel using the new Thing-based graph system

use crate::interrupts::init_interrupts;
use crate::os_space::{HHDM_REQUEST, MEMMAP_REQUEST};
use crate::thing::{Fact, Predicate, Space, Thing, Uri};
use crate::{does, dump_overlay};
use crate::{
    framebuffer::Framebuffer,
    gdt::init_gdt,
    idt::{init_double_fault_stack, init_idt},
    memory::{self, HEAP_SIZE, HEAP_START},
    serial_println,
};
use alloc::{boxed::Box, format, sync::Arc};
use core::arch::asm;
use limine::request::HhdmRequest;
use spin::Mutex;
use x86_64::{
    VirtAddr,
    structures::paging::{FrameAllocator, Mapper},
};

pub const USER_BASE_VADDR: u64 = 0x4000_0000;

pub struct Kernel {
    pub graph: Arc<Mutex<dyn Space + 'static>>,
    mapper: &'static mut x86_64::structures::paging::OffsetPageTable<'static>,
    frame_allocator: memory::BootFrameAllocator,
    framebuffer: Framebuffer,
}

fn get_hhdm_offset() -> VirtAddr {
    let resp = HHDM_REQUEST.get_response().expect("No HHDM response");
    let offset = resp.offset();
    serial_println!("HHDM offset: {:#x}", offset);
    VirtAddr::new(offset)
}

impl Kernel {
    pub fn spin(&mut self) -> ! {
        serial_println!("Kernel spin loop started");
        let mut x: usize = 0;
        let mut direction: isize = 1;
        let mut tick: u64 = 0;

        loop {
            self.framebuffer.draw_rect(x, 100, 100, 50, 0x00FF00);

            let msg = format!("tick: {}", tick);
            self.framebuffer.draw_text(&msg, x, 200, 20);
            self.framebuffer.draw_text("ThingOS Kernel UI", 20, 20, 24);
            self.framebuffer
                .draw_text("Dragons not yet implemented", 20, 50, 16);

            self.framebuffer.flush();

            x = (x as isize + direction) as usize;
            if x > self.framebuffer.width - 100 || x == 0 {
                direction *= -1;
            }

            tick = tick.wrapping_add(1);
        }
    }

    pub fn new() -> Self {
        serial_println!("Creating new Kernel instance");
        let (mapper, frame_allocator) = Self::init_memory();
        let graph = Self::init_graph_space();
        let framebuffer = Self::init_framebuffer();

        let mut kernel = Kernel {
            graph,
            mapper,
            frame_allocator,
            framebuffer,
        };

        kernel.populate_graph();
        kernel.map_bootloader_memory();
        kernel.print();

        kernel
    }

    fn init_memory() -> (
        &'static mut x86_64::structures::paging::OffsetPageTable<'static>,
        memory::BootFrameAllocator,
    ) {
        serial_println!("Initializing memory");
        let offset = get_hhdm_offset();
        let (mut mapper, mut frame_allocator) = unsafe { memory::init(offset) };
        serial_println!("Paging initialized");

        init_gdt();
        serial_println!("GDT initialized");

        #[allow(static_mut_refs)]
        let tss = unsafe { crate::gdt::TSS.as_mut().expect("TSS not initialized") };
        init_double_fault_stack(tss, &mut mapper, &mut frame_allocator);
        serial_println!("Double fault stack initialized");

        init_idt();
        init_interrupts();
        x86_64::instructions::interrupts::enable();
        serial_println!("IDT and interrupts initialized");

        (mapper, frame_allocator)
    }

    fn init_graph_space() -> Arc<Mutex<dyn Space>> {
        serial_println!("Initializing graph space");
        Arc::new(Mutex::new(crate::os_space::OsSpace::new()))
    }

    fn init_framebuffer() -> Framebuffer {
        serial_println!("Initializing framebuffer");
        Framebuffer::new().expect("Framebuffer not initialized")
    }

    fn populate_graph(&mut self) {
        serial_println!("Populating graph with initial facts");
        let mut graph = self.graph.lock();
        let kernel = Uri("os://kernel".into());
        let screen = Uri("ui://screen/1".into());
        let process = Uri("proc://1".into());

        graph
            .assert(Fact::that(
                kernel.clone(),
                does!("is"),
                Uri("kind:kernel".into()),
            ))
            .unwrap();
        graph
            .assert(Fact::that(
                screen.clone(),
                does!("is"),
                Uri("kind:screen".into()),
            ))
            .unwrap();
        graph
            .assert(Fact::that(
                process.clone(),
                does!("is"),
                Uri("kind:process".into()),
            ))
            .unwrap();

        graph
            .assert(Fact::that(kernel.clone(), does!("spawn"), process.clone()))
            .unwrap();
        graph
            .assert(Fact::that(kernel.clone(), does!("draw_to"), screen.clone()))
            .unwrap();
        graph
            .assert(Fact::that(process.clone(), does!("open"), screen.clone()))
            .unwrap();
    }

    fn map_bootloader_memory(&mut self) {
        serial_println!("Mapping bootloader memory to graph");
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
        serial_println!("Memory mapped range: base={:#x}, len={:#x}", base, len);

        let slice = unsafe { core::slice::from_raw_parts(base as *const u8, len as usize) };
        let mut graph = self.graph.lock();
        graph.write_content(&Uri("os://kernel".into()), slice).ok();
    }

    fn print(&self) {
        serial_println!("Printing Thing facts");
        let thing = Thing {
            uri: Uri("os://kernel".into()),
            space: self.graph.clone(),
        };
        for fact in thing.facts() {
            serial_println!("{} {} {}", fact.this.0, fact.predicate.0, fact.that.0);
        }
        let graph = self.graph.lock();
        dump_overlay!(&*graph);
    }
}
