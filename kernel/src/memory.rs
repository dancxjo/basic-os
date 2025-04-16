use crate::serial_println;
use crate::thing::Thingable;
use alloc::vec::Vec;
use limine::{memory_map::EntryType, request::MemoryMapRequest};
use linked_list_allocator::LockedHeap;
use serde::{Deserialize, Serialize};
use thing_macros::Thing;
use x86_64::{
    VirtAddr,
    registers::control::Cr3,
    structures::paging::{OffsetPageTable, PageTable},
};

const HEAP_SIZE_IN_MIBS: usize = 4;
const HEAP_SIZE_IN_BYTES: usize = HEAP_SIZE_IN_MIBS * 1024 * 1024;
static mut HEAP: [u8; HEAP_SIZE_IN_BYTES] = [0; HEAP_SIZE_IN_BYTES];
pub const HEAP_START: u64 = 0x_4444_4444_0000;
pub const HEAP_SIZE: usize = 16 * 1024 * 1024; // 16 MiB

#[global_allocator]
static ALLOCATOR: LockedHeap = LockedHeap::empty();

pub fn init_initial_allocator() {
    #[allow(static_mut_refs)]
    unsafe {
        ALLOCATOR.lock().init(HEAP.as_mut_ptr(), HEAP_SIZE_IN_BYTES);
    }
}

use core::{mem::MaybeUninit, range::Range};

static mut MAPPER: MaybeUninit<OffsetPageTable> = MaybeUninit::uninit();

pub unsafe fn init_paging(
    physical_memory_offset: VirtAddr,
) -> &'static mut OffsetPageTable<'static> {
    serial_println!("Initializing paging...");
    let l4_table = unsafe { active_level_4_table(physical_memory_offset) };
    serial_println!("L4 table acquired");
    let page_table_root = unsafe { OffsetPageTable::new(l4_table, physical_memory_offset) };
    #[allow(static_mut_refs)]
    unsafe {
        MAPPER.write(page_table_root);
    }
    #[allow(static_mut_refs)]
    unsafe {
        MAPPER.assume_init_mut()
    }
}

unsafe fn active_level_4_table(offset: VirtAddr) -> &'static mut PageTable {
    let (frame, _) = Cr3::read();
    let phys = frame.start_address();
    let virt = offset + phys.as_u64();
    unsafe { &mut *(virt.as_mut_ptr()) }
}

#[used]
static MEMMAP_REQUEST: MemoryMapRequest = MemoryMapRequest::new();

#[derive(Clone, Copy, Debug)]
pub struct MemoryRegion {
    pub base: u64,
    pub len: u64,
    pub kind: &'static str,
}

static mut CACHED_REGIONS: Option<&'static [MemoryRegion]> = None;

fn collect_memory_regions() -> &'static [MemoryRegion] {
    static mut REGIONS: [MemoryRegion; 128] = [MemoryRegion {
        base: 0,
        len: 0,
        kind: "unknown",
    }; 128];

    unsafe {
        if let Some(cached) = CACHED_REGIONS {
            return cached;
        }

        let response = MEMMAP_REQUEST
            .get_response()
            .expect("No memory map from Limine");
        let entries = response.entries();

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

            REGIONS[count] = MemoryRegion {
                base: entry.base,
                len: entry.length,
                kind,
            };
            count += 1;
        }

        let result = &REGIONS[..count];
        CACHED_REGIONS = Some(result);
        result
    }
}

#[derive(Debug, Serialize, Deserialize, Thing)]
pub struct BootFrameAllocator {
    pub usable_ranges: Vec<MemoryRange>,
    pub current_range: usize,
    pub next: usize,
}

impl BootFrameAllocator {
    pub fn new(usable_ranges: Vec<MemoryRange>) -> Self {
        BootFrameAllocator {
            usable_ranges,
            current_range: 0,
            next: 0,
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct MemoryRange {
    pub start: usize,
    pub end: usize,
}

impl From<Range<usize>> for MemoryRange {
    fn from(range: Range<usize>) -> Self {
        MemoryRange {
            start: range.start,
            end: range.end,
        }
    }
}

impl Into<core::ops::Range<usize>> for MemoryRange {
    fn into(self) -> core::ops::Range<usize> {
        self.start..self.end
    }
}

pub fn print_memory_regions() {
    let regions = collect_memory_regions();
    serial_println!("Available memory regions:");
    for region in regions {
        serial_println!("{:?}", region);
    }
}
