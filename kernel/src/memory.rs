use crate::serial_println;
use limine::{memory_map::EntryType, request::MemoryMapRequest};
use linked_list_allocator::LockedHeap;
use x86_64::{
    PhysAddr, VirtAddr,
    registers::control::Cr3,
    structures::paging::{
        Mapper, OffsetPageTable, Page, PageTable, PageTableFlags, PhysFrame, Size4KiB,
    },
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

use x86_64::structures::paging::PageTableFlags as Flags;

pub fn init_heap(mapper: &mut OffsetPageTable, frame_allocator: &mut BootFrameAllocator) {
    let heap_start = VirtAddr::new(HEAP_START);
    let heap_end = heap_start + HEAP_SIZE as u64;

    let start_page: Page<Size4KiB> = Page::containing_address(heap_start);
    let end_page: Page<Size4KiB> = Page::containing_address(heap_end - 1u64);

    for page in Page::range_inclusive(start_page, end_page) {
        let frame = frame_allocator
            .allocate_frame()
            .expect("Out of physical frames for heap");
        let phys_frame = PhysFrame::from_start_address(PhysAddr::new(frame as u64))
            .expect("Failed to create PhysFrame");
        unsafe {
            mapper
                .map_to(
                    page,
                    phys_frame,
                    Flags::PRESENT | Flags::WRITABLE,
                    frame_allocator,
                )
                .expect("Heap page mapping failed")
                .flush();
        }
    }

    unsafe {
        ALLOCATOR.lock().init(HEAP_START as *mut u8, HEAP_SIZE);
    }

    serial_println!(
        "Heap initialized at {:#x} - {:#x}",
        HEAP_START,
        HEAP_START + HEAP_SIZE as u64
    );
}

#[allow(static_mut_refs)]
pub unsafe fn init_paging(
    physical_memory_offset: VirtAddr,
) -> &'static mut OffsetPageTable<'static> {
    serial_println!("Initializing paging...");
    let l4_table = unsafe { active_level_4_table(physical_memory_offset) };
    serial_println!("L4 table acquired");
    unsafe {
        MAPPER.write(OffsetPageTable::new(l4_table, physical_memory_offset));
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

pub struct BootFrameAllocator {
    usable_ranges: [Option<Range<usize>>; 32],
    range_count: usize,
    current_range: usize,
    next: usize,
}

static mut INSTANCE: Option<BootFrameAllocator> = None;

#[allow(static_mut_refs)]
impl BootFrameAllocator {
    pub fn init() -> &'static mut Self {
        unsafe {
            INSTANCE.get_or_insert_with(|| {
                let regions = collect_memory_regions();
                let mut usable_ranges: [Option<core::range::Range<usize>>; 32] = [None; 32];
                let mut range_count = 0;

                for r in regions
                    .iter()
                    .filter(|r| r.kind == "usable" && r.len >= 0x200000)
                {
                    if range_count >= usable_ranges.len() {
                        break;
                    }
                    usable_ranges[range_count] =
                        Some(((r.base as usize)..(r.base + r.len) as usize).into());
                    range_count += 1;
                }

                assert!(range_count > 0, "No usable memory regions!");

                usable_ranges[..range_count].sort_by_key(|range| {
                    usize::MAX - (range.as_ref().unwrap().end - range.as_ref().unwrap().start)
                });

                let first_range = usable_ranges[0].as_ref().unwrap();
                let next = first_range.start;

                BootFrameAllocator {
                    usable_ranges,
                    range_count,
                    current_range: 0,
                    next,
                }
            })
        }
    }

    fn allocate_frame(&mut self) -> Option<usize> {
        loop {
            if self.current_range >= self.range_count {
                return None;
            }

            let current_range = self.usable_ranges[self.current_range].as_ref().unwrap();
            let aligned = (self.next + 0xFFF) & !0xFFF;

            if aligned + 0x1000 <= current_range.end {
                self.next = aligned + 0x1000;
                return Some(aligned);
            } else {
                self.current_range += 1;
                if self.current_range < self.range_count {
                    self.next = self.usable_ranges[self.current_range]
                        .as_ref()
                        .unwrap()
                        .start;
                }
            }
        }
    }

    pub fn allocate_and_map(
        &mut self,
        mapper: &mut OffsetPageTable,
        physical_memory_offset: VirtAddr,
    ) -> Option<PhysFrame> {
        let addr = self.allocate_frame()?;
        let frame = PhysFrame::from_start_address(PhysAddr::new(addr as u64)).unwrap();
        let virt = physical_memory_offset + addr as u64;
        let page = Page::containing_address(virt);
        unsafe {
            mapper
                .map_to(
                    page,
                    frame,
                    PageTableFlags::PRESENT | PageTableFlags::WRITABLE,
                    self,
                )
                .ok()?
                .flush();
        }
        Some(frame)
    }
}

unsafe impl x86_64::structures::paging::FrameAllocator<Size4KiB> for BootFrameAllocator {
    fn allocate_frame(&mut self) -> Option<PhysFrame> {
        let addr = self.allocate_frame()?;
        let frame = PhysFrame::from_start_address(PhysAddr::new(addr as u64)).unwrap();

        // Ensure identity mapping for page table frames
        let virt = addr as u64; // identity map offset
        let page = Page::containing_address(VirtAddr::new(virt));
        #[allow(static_mut_refs)]
        let mapper = unsafe { &mut *MAPPER.as_mut_ptr() };

        unsafe {
            let _ = mapper
                .map_to(
                    page,
                    frame,
                    PageTableFlags::PRESENT | PageTableFlags::WRITABLE,
                    self,
                )
                .ok()?
                .flush();
        }

        Some(frame)
    }
}

pub fn print_memory_regions() {
    let regions = collect_memory_regions();
    serial_println!("Available memory regions:");
    for region in regions {
        serial_println!("{:?}", region);
    }
}
