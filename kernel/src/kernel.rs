// kernel.rs
extern crate alloc;

use crate::bootloader::thingify_boot_modules;
use crate::framebuffer::Framebuffer;
use crate::graph::Graph;
use crate::helpers::thingify::{
    collect_memory_regions, thingify_boot_memory, thingify_pci_devices, thingify_ui_layout,
};
use crate::keyboard::Keyboard;
use crate::memory::{self, BootFrameAllocator};
use crate::mouse::Mouse;
use crate::stem::Stem;
// use crate::stem::Stem;
use crate::things::thingable::Thingable;
// use alloc::vec;
use limine::request::HhdmRequest;
use thing_macros::Thing;
use x86_64::structures::paging::{Mapper, Page, PageTableFlags, PhysFrame, Size4KiB};
use x86_64::{PhysAddr, VirtAddr};

use alloc::{boxed::Box, format, vec::Vec};

use limine::memory_map::EntryType;
use limine::request::MemoryMapRequest;
use tinypci::{PciDeviceInfo, brute_force_scan};

#[used]
static MEMMAP_REQUEST: MemoryMapRequest = MemoryMapRequest::new();

unsafe extern "C" {
    static _start: u8;
    static _etext: u8;
    static _edata: u8;
    static _end: u8;
}
pub fn thingify_memory_region(
    graph: &mut Graph,
    name: &'static str,
    kind: &'static str,
    start: usize,
    length: usize,
) -> usize {
    graph.add_kind(kind, ""); // idempotent
    let view = RegionView {
        base: start,
        length,
    };
    graph.create_typed(name, kind, view)
}

pub fn thingify_kernel(graph: &mut Graph) {
    graph.add_kind("process", "A running unit of code");
    graph.add_kind("segment", "Code or data segment");
    graph.add_predicate("contains", "process", "segment");

    let kernel_id = graph.create_typed("kernel", "process", ());

    let mut seg = |name, start: *const u8, end: *const u8| {
        let len = end as usize - start as usize;
        let seg_id = thingify_memory_region(graph, name, "segment", start as usize, len);
        graph.link(kernel_id, seg_id, "contains");
    };

    unsafe {
        seg("kernel.text", &_start, &_etext);
        seg("kernel.data", &_etext, &_edata);
        seg("kernel.bss", &_edata, &_end);
    }
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct RegionView {
    pub base: usize,
    pub length: usize,
}

#[derive(Thing, Clone)]
pub struct Region {
    pub base: usize,
    pub length: usize,
}

pub struct Kernel {
    graph: Graph,
    fb: Framebuffer,
    keyboard: Keyboard,
    mouse: Mouse,
    allocator: BootFrameAllocator,
}
impl Kernel {
    pub fn init() -> Self {
        #[allow(static_mut_refs)]
        unsafe {
            crate::serial::SERIAL1.init()
        };
        memory::init_heap();
        let graph = Graph::new();
        let fb = Framebuffer::init().expect("No framebuffer");
        let keyboard = Keyboard::init();
        let mouse = Mouse::init();
        let allocator = BootFrameAllocator::from_graph(&graph);

        let mut kernel = Self {
            graph,
            fb,
            keyboard,
            mouse,
            allocator,
        };

        kernel.init_graph();
        kernel.allocator = BootFrameAllocator::from_graph(&kernel.graph);
        kernel.init_paging();
        kernel
    }

    fn init_graph(&mut self) {
        thingify_kernel(&mut self.graph);
        let regions = collect_memory_regions();
        thingify_boot_memory(&mut self.graph, regions);
        thingify_boot_modules(&mut self.graph);
        thingify_pci_devices(&mut self.graph);
        thingify_ui_layout(&mut self.graph);
    }

    fn init_paging(&mut self) {
        static HHDM: HhdmRequest = HhdmRequest::new();
        let offset = HHDM.get_response().as_ref().unwrap().offset();
        let phys_offset = VirtAddr::new(offset);

        let mapper = unsafe { memory::init_paging(phys_offset) };

        let heap_start = VirtAddr::new(0xffff_ffff_0000_0000);
        for i in 0..16 {
            let page = Page::<Size4KiB>::containing_address(heap_start + i * 0x1000);
            let frame = self.allocator.allocate_frame().expect("Out of frames");
            let phys = PhysAddr::new(frame as u64);
            let phys_frame = PhysFrame::containing_address(phys);
            unsafe {
                mapper
                    .map_to(
                        page,
                        phys_frame,
                        PageTableFlags::PRESENT | PageTableFlags::WRITABLE,
                        &mut self.allocator,
                    )
                    .expect("map_to failed")
                    .flush();
            }
        }
    }

    pub fn run(mut self) -> ! {
        Stem::draw(&self.graph, &mut self.fb);

        loop {
            // input, logging, drawing...
            self.fb.flush();
        }
    }
}
