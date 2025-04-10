use alloc::{boxed::Box, vec::Vec};
use linked_list_allocator::LockedHeap;
use x86_64::{
    PhysAddr,
    structures::paging::{FrameAllocator, PhysFrame, Size4KiB},
};
use x86_64::{
    VirtAddr,
    registers::control::Cr3,
    structures::paging::{OffsetPageTable, PageTable},
};

use crate::{graph::Graph, helpers::thingify::RegionView, println};

// Define the heap size
const HEAP_SIZE: usize = 8 * 1024 * 1024;

#[repr(align(16))]
struct AlignedHeap([u8; HEAP_SIZE]);

static mut HEAP_SPACE: AlignedHeap = AlignedHeap([0; HEAP_SIZE]);

#[global_allocator]
static ALLOCATOR: LockedHeap = LockedHeap::empty();

#[allow(static_mut_refs)]
pub fn init_heap() {
    unsafe {
        let heap_start = HEAP_SPACE.0.as_ptr() as *mut u8;
        ALLOCATOR.lock().init(heap_start, HEAP_SIZE);
    }
}

/// Initialize paging and return an OffsetPageTable
/// `physical_memory_offset` is the offset between virtual and physical memory (from Limine)
pub unsafe fn init_paging(
    physical_memory_offset: VirtAddr,
) -> &'static mut OffsetPageTable<'static> {
    let level_4_table = unsafe { active_level_4_table(physical_memory_offset) };
    let offset_page_table = unsafe { OffsetPageTable::new(level_4_table, physical_memory_offset) };
    &mut *(Box::leak(Box::new(offset_page_table)))
}

/// Get a mutable reference to the active level 4 page table
unsafe fn active_level_4_table(physical_memory_offset: VirtAddr) -> &'static mut PageTable {
    let (level_4_frame, _) = Cr3::read();
    let phys = level_4_frame.start_address();
    let virt = physical_memory_offset + phys.as_u64();
    let page_table_ptr: *mut PageTable = virt.as_mut_ptr();
    unsafe { &mut *page_table_ptr }
}

pub struct BootFrameAllocator {
    usable_ranges: Vec<(usize, usize)>, // (start, end) of each usable region
    next: usize,
}

impl BootFrameAllocator {
    pub fn from_graph(graph: &Graph) -> Self {
        let mut ranges = Vec::new();
        for thing in &graph.things {
            if thing.kind == "region" && thing.name.contains(".usable") {
                if let Some(view) = thing.data.as_typed::<RegionView>() {
                    println!("Base: {:x}, Length: {}", view.base, view.length);
                    ranges.push((view.base, view.base + view.length));
                }
            }
        }
        ranges.sort(); // Ensure ascending
        BootFrameAllocator {
            usable_ranges: ranges,
            next: 0,
        }
    }

    pub fn allocate_frame(&mut self) -> Option<usize> {
        while let Some(&(start, end)) = self.usable_ranges.get(0) {
            let aligned = (self.next + 0xFFF) & !0xFFF;
            if aligned + 0x1000 <= end {
                self.next = aligned + 0x1000;
                return Some(aligned);
            } else {
                self.usable_ranges.remove(0);
            }
        }
        None
    }
}

unsafe impl FrameAllocator<Size4KiB> for BootFrameAllocator {
    fn allocate_frame(&mut self) -> Option<PhysFrame> {
        self.allocate_frame()
            .map(|addr| PhysFrame::from_start_address(PhysAddr::new(addr as u64)).unwrap())
    }
}
