//! memory.rs — ThingOS memory management: paging, allocator, and memory map

use alloc::boxed::Box;
use core::{mem::MaybeUninit, ops::Range};
use limine::memory_map::{Entry, EntryType};
use linked_list_allocator::LockedHeap;
use x86_64::{
    PhysAddr, VirtAddr,
    registers::control::Cr3,
    structures::paging::{
        FrameAllocator, Mapper, OffsetPageTable, Page, PageTable, PageTableFlags, PhysFrame,
        Size4KiB,
    },
};

use crate::serial_println;

/// Virtual heap location and size (mapped by the kernel)
pub const HEAP_START: u64 = 0x_4444_4444_0000;
pub const HEAP_SIZE: usize = 64 * 1024 * 1024;

/// Global heap allocator used by Box, Vec, etc.
#[global_allocator]
static ALLOCATOR: LockedHeap = LockedHeap::empty();

/// Max number of usable memory ranges to track (early boot)
const MAX_RANGES: usize = 64;
static mut FRAME_RANGES: [MaybeUninit<Range<PhysAddr>>; MAX_RANGES] = {
    const UNINIT: MaybeUninit<Range<PhysAddr>> = MaybeUninit::uninit();
    [UNINIT; MAX_RANGES]
};

/// Boot-time frame allocator using static memory (no heap)
pub struct BootFrameAllocator {
    ranges: &'static mut [Range<PhysAddr>],
    current_range: usize,
}

impl BootFrameAllocator {
    pub fn new(entries: &[Entry]) -> Self {
        let mut count = 0;
        unsafe {
            for entry in entries.iter() {
                if entry.entry_type == EntryType::USABLE && count < MAX_RANGES {
                    let start = PhysAddr::new(entry.base);
                    let end = PhysAddr::new(entry.base + entry.length);
                    FRAME_RANGES[count].write(start..end);
                    count += 1;
                }
            }

            #[allow(static_mut_refs)]
            let slice = core::slice::from_raw_parts_mut(
                FRAME_RANGES.as_mut_ptr() as *mut Range<PhysAddr>,
                count,
            );

            BootFrameAllocator {
                ranges: slice,
                current_range: 0,
            }
        }
    }
}

unsafe impl FrameAllocator<Size4KiB> for BootFrameAllocator {
    fn allocate_frame(&mut self) -> Option<PhysFrame> {
        while self.current_range < self.ranges.len() {
            let range = &mut self.ranges[self.current_range];
            let start = align_up(range.start.as_u64(), 0x1000);

            if start < range.end.as_u64() {
                let frame = PhysFrame::containing_address(PhysAddr::new(start));
                range.start = PhysAddr::new(start + 0x1000);
                return Some(frame);
            } else {
                self.current_range += 1;
            }
        }
        None
    }
}

/// Aligns `addr` up to the nearest multiple of `align`
fn align_up(addr: u64, align: u64) -> u64 {
    (addr + align - 1) & !(align - 1)
}

/// Initializes memory: paging, heap, and frame allocator
pub unsafe fn init(
    hhdm_offset: VirtAddr,
) -> (&'static mut OffsetPageTable<'static>, BootFrameAllocator) {
    serial_println!("Initializing memory...");

    let level_4_table = unsafe { active_level_4_table(hhdm_offset) };
    let mut mapper = unsafe { OffsetPageTable::new(level_4_table, hhdm_offset) };
    let memory_entries = get_memory_entries_copy();
    let mut frame_allocator = BootFrameAllocator::new(memory_entries);

    map_heap(&mut mapper, &mut frame_allocator);
    unsafe { ALLOCATOR.lock().init(HEAP_START as *mut u8, HEAP_SIZE) };

    (Box::leak(Box::new(mapper)), frame_allocator)
}

/// Get mutable access to the active level 4 page table
unsafe fn active_level_4_table(phys_offset: VirtAddr) -> &'static mut PageTable {
    let (level_4_phys, _) = Cr3::read();
    let virt = phys_offset + level_4_phys.start_address().as_u64();
    unsafe { &mut *(virt.as_mut_ptr()) }
}

/// Maps the heap region into virtual memory using 4 KiB pages
fn map_heap<M: Mapper<Size4KiB>, F: FrameAllocator<Size4KiB>>(
    mapper: &mut M,
    frame_allocator: &mut F,
) {
    let heap_start = VirtAddr::new(HEAP_START);
    let heap_end = heap_start + HEAP_SIZE as u64;
    let page_range = Page::range_inclusive(
        Page::containing_address(heap_start),
        Page::containing_address(VirtAddr::new(heap_end.as_u64() - 1)),
    );

    for page in page_range {
        let frame = frame_allocator
            .allocate_frame()
            .expect("Out of physical frames during heap mapping!");
        let flags = PageTableFlags::PRESENT | PageTableFlags::WRITABLE;

        unsafe {
            mapper
                .map_to(page, frame, flags, frame_allocator)
                .expect("map_to failed")
                .flush();
        }
    }
}

use limine::request::MemoryMapRequest;

const MAX_ENTRIES: usize = 64;
static mut MEMORY_ENTRY_COPY: [Entry; MAX_ENTRIES] = [Entry {
    base: 0,
    length: 0,
    entry_type: EntryType::RESERVED,
}; MAX_ENTRIES];

#[used]
static MEMORY_MAP_REQUEST: MemoryMapRequest = MemoryMapRequest::new();

pub fn get_memory_entries_copy() -> &'static [Entry] {
    let response = unsafe {
        MEMORY_MAP_REQUEST
            .get_response()
            .expect("No memory map response from Limine")
    };

    let entries = response.entries();
    let mut count = 0;

    unsafe {
        for &entry_ref in entries.iter().take(MAX_ENTRIES) {
            MEMORY_ENTRY_COPY[count] = Entry {
                base: entry_ref.base,
                length: entry_ref.length,
                entry_type: entry_ref.entry_type,
            };
            count += 1;
        }

        &MEMORY_ENTRY_COPY[..count]
    }
}
