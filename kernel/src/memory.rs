#![allow(dead_code)]

extern crate alloc;

use alloc::{boxed::Box, vec::Vec};
use core::ops::Range;
use limine::{memory_map::EntryType, request::MemoryMapRequest};
// use limine::{MemoryMapEntryType as EntryType, MemoryMapRequest};

use linked_list_allocator::LockedHeap;
use x86_64::{
    PhysAddr, VirtAddr,
    registers::control::Cr3,
    structures::paging::{
        FrameAllocator, Mapper, OffsetPageTable, Page, PageTable, PageTableFlags as Flags,
        PhysFrame, Size4KiB,
    },
};

use crate::serial_println;

// --- Constants ------------------------------------------------------

pub const HEAP_START: u64 = 0x_4444_4444_0000;
pub const HEAP_SIZE: usize = 16 * 1024 * 1024; // 16 MiB

// --- Global Allocator Setup -----------------------------------------

#[global_allocator]
static GLOBAL_ALLOCATOR: LockedHeap = LockedHeap::empty();

/// Sets up paging, frame allocator, and the heap.
/// Call this once in `kmain()`.
pub fn init(physical_memory_offset: VirtAddr) -> &'static mut OffsetPageTable<'static> {
    let mapper = unsafe { init_paging(physical_memory_offset) };
    let frame_allocator = BootFrameAllocator::init();

    map_heap(
        mapper,
        frame_allocator,
        VirtAddr::new(HEAP_START),
        HEAP_SIZE,
    );
    unsafe {
        GLOBAL_ALLOCATOR
            .lock()
            .init(HEAP_START as *mut u8, HEAP_SIZE);
    }

    serial_println!("[mem] Paging, heap, and frame allocator initialized.");
    mapper
}

// --- Paging Setup ---------------------------------------------------

unsafe fn init_paging(physical_memory_offset: VirtAddr) -> &'static mut OffsetPageTable<'static> {
    unsafe {
        let l4_table = active_level_4_table(physical_memory_offset);
        let mapper = OffsetPageTable::new(l4_table, physical_memory_offset);
        Box::leak(Box::new(mapper))
    }
}

unsafe fn active_level_4_table(offset: VirtAddr) -> &'static mut PageTable {
    let (frame, _) = Cr3::read();
    let phys = frame.start_address();
    let virt = offset + phys.as_u64();
    unsafe { &mut *(virt.as_mut_ptr()) }
}

fn map_heap(
    mapper: &mut OffsetPageTable,
    frame_allocator: &mut impl FrameAllocator<Size4KiB>,
    heap_start: VirtAddr,
    heap_size: usize,
) {
    let start = Page::containing_address(heap_start);
    let end = Page::containing_address(heap_start + heap_size as u64 - 1u64);
    let page_range = Page::range_inclusive(start, end);

    for page in page_range {
        let frame = frame_allocator
            .allocate_frame()
            .expect("Out of physical frames!");
        unsafe {
            mapper
                .map_to(
                    page,
                    frame,
                    Flags::PRESENT | Flags::WRITABLE,
                    frame_allocator,
                )
                .expect("map_to failed")
                .flush();
        }
    }
}

// --- Memory Map Request ---------------------------------------------

#[used]
static MEMMAP_REQUEST: MemoryMapRequest = MemoryMapRequest::new();

#[derive(Clone, Copy, Debug)]
pub struct MemoryRegion {
    pub base: u64,
    pub len: u64,
    pub kind: &'static str,
}

fn collect_memory_regions() -> &'static [MemoryRegion] {
    let response = MEMMAP_REQUEST
        .get_response()
        .expect("No memory map from Limine");
    let entries = response.entries();

    static mut REGIONS: [MemoryRegion; 128] = [MemoryRegion {
        base: 0,
        len: 0,
        kind: "unknown",
    }; 128];

    let mut count = 0;

    for entry in entries {
        let kind = match entry.entry_type {
            EntryType::USABLE => "usable",
            EntryType::RESERVED => "reserved",
            EntryType::ACPI_RECLAIMABLE => "acpi_reclaimable",
            EntryType::ACPI_NVS => "acpi_nvs",
            EntryType::BAD_MEMORY => "bad_memory",
            EntryType::BOOTLOADER_RECLAIMABLE => "bootloader_reclaimable",
            EntryType::FRAMEBUFFER => "framebuffer",
            _ => "unknown",
        };

        unsafe {
            REGIONS[count] = MemoryRegion {
                base: entry.base,
                len: entry.length,
                kind,
            };
            count += 1;
        }
    }

    unsafe { &REGIONS[..count] }
}

// --- Boot-Time Frame Allocator --------------------------------------

pub struct BootFrameAllocator {
    usable_ranges: Vec<Range<usize>>,
    next: usize,
}

static mut INSTANCE: Option<BootFrameAllocator> = None;

#[allow(static_mut_refs)]
impl BootFrameAllocator {
    pub fn init() -> &'static mut Self {
        unsafe {
            INSTANCE.get_or_insert_with(|| {
                let regions = collect_memory_regions();
                let usable_ranges = regions
                    .iter()
                    .filter(|r| r.kind == "usable")
                    .map(|r| (r.base as usize)..(r.base + r.len) as usize)
                    .collect();

                BootFrameAllocator {
                    usable_ranges,
                    next: 0,
                }
            })
        }
    }

    fn allocate_frame(&mut self) -> Option<usize> {
        while let Some(range) = self.usable_ranges.get_mut(0) {
            let aligned = (self.next + 0xFFF) & !0xFFF;

            if aligned + 0x1000 <= range.end {
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
