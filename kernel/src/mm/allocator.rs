use core::mem::MaybeUninit;
use core::ops::Range;
use linked_list_allocator::LockedHeap;
use x86_64::{
    PhysAddr, VirtAddr,
    registers::control::Cr3,
    structures::paging::{
        FrameAllocator, Mapper, OffsetPageTable, Page, PageTable, PageTableFlags, PhysFrame,
        Size4KiB, Translate,
    },
};

use crate::bootloader::collect_memory_regions;

// Constants for heap placement

pub const HEAP_START: u64 = 0x4444_4444_0000;
pub const HEAP_SIZE: usize = 32 * 1024 * 1024;

#[global_allocator]
static ALLOCATOR: LockedHeap = LockedHeap::empty();

pub(crate) static mut MAPPER: MaybeUninit<OffsetPageTable> = MaybeUninit::uninit();
static mut FRAME_ALLOCATOR: MaybeUninit<BootFrameAllocator> = MaybeUninit::uninit();

/// Initialize paging and return the active OffsetPageTable.
pub unsafe fn init_paging(
    physical_memory_offset: VirtAddr,
) -> &'static mut OffsetPageTable<'static> {
    let l4_table = unsafe { active_level_4_table(physical_memory_offset) };
    unsafe {
        #[allow(static_mut_refs)]
        MAPPER.write(OffsetPageTable::new(l4_table, physical_memory_offset));
        #[allow(static_mut_refs)]
        MAPPER.assume_init_mut()
    }
}

unsafe fn active_level_4_table(offset: VirtAddr) -> &'static mut PageTable {
    let (frame, _) = Cr3::read();
    let phys = frame.start_address();
    let virt = offset + phys.as_u64();
    unsafe { &mut *(virt.as_mut_ptr()) }
}

/// Boot frame allocator
pub struct BootFrameAllocator {
    usable_ranges: [Option<Range<usize>>; 32],
    range_count: usize,
    current_range: usize,
    next: usize,
}

impl BootFrameAllocator {
    pub fn init() -> &'static mut Self {
        unsafe {
            #[allow(static_mut_refs)]
            FRAME_ALLOCATOR.write(Self::new());
            #[allow(static_mut_refs)]
            FRAME_ALLOCATOR.assume_init_mut()
        }
    }

    fn new() -> Self {
        let regions = collect_memory_regions();
        let mut usable_ranges: [Option<Range<usize>>; 32] = Default::default();
        let mut range_count = 0;

        for r in regions
            .iter()
            .filter(|r| r.kind == "usable" && r.len >= 0x200000)
        {
            if range_count >= usable_ranges.len() {
                break;
            }
            usable_ranges[range_count] = Some((r.base as usize)..(r.base + r.len) as usize);
            range_count += 1;
        }

        assert!(range_count > 0, "No usable memory regions found!");

        usable_ranges[..range_count].sort_by_key(|range| {
            usize::MAX - (range.as_ref().unwrap().end - range.as_ref().unwrap().start)
        });

        let first_range_start = usable_ranges[0].as_ref().unwrap().start;

        log::info!(
            "BootFrameAllocator initialized with {} usable ranges",
            range_count
        );

        Self {
            usable_ranges,
            range_count,
            current_range: 0,
            next: first_range_start,
        }
    }

    fn allocate_frame_internal(&mut self) -> Option<usize> {
        loop {
            if self.current_range >= self.range_count {
                return None;
            }

            let current_range = self.usable_ranges[self.current_range].as_ref().unwrap();
            let aligned = (self.next + 0xFFF) & !0xFFF;

            if aligned + 0x1000 <= current_range.end {
                self.next = aligned + 0x1000;

                // if self.used_frames.contains(&aligned) {
                //     continue;
                // }

                // self.used_frames.insert(aligned);
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
    /// Manually expose frame allocator for other systems
    pub fn global() -> &'static mut Self {
        #[allow(static_mut_refs)]
        unsafe {
            FRAME_ALLOCATOR.assume_init_mut()
        }
    }
}

/// Obtain the global page-table mapper initialized during boot.
pub fn global_mapper() -> &'static mut OffsetPageTable<'static> {
    #[allow(static_mut_refs)]
    unsafe {
        MAPPER.assume_init_mut()
    }
}

unsafe impl FrameAllocator<Size4KiB> for BootFrameAllocator {
    fn allocate_frame(&mut self) -> Option<PhysFrame> {
        let addr = self.allocate_frame_internal()?;
        PhysFrame::from_start_address(PhysAddr::new(addr as u64)).ok()
    }
}

/// Check if a virtual address is already mapped
pub fn is_mapped(mapper: &OffsetPageTable, addr: VirtAddr) -> bool {
    mapper.translate_addr(addr).is_some()
}

/// Initialize and map the heap
pub fn init_heap(mapper: &mut OffsetPageTable, frame_allocator: &mut BootFrameAllocator) {
    let heap_start = VirtAddr::new(HEAP_START);
    let heap_end = heap_start + HEAP_SIZE as u64;

    let start_page = Page::containing_address(heap_start);
    let end_page = Page::containing_address(heap_end - 1u64);

    for page in Page::range_inclusive(start_page, end_page) {
        if is_mapped(mapper, page.start_address()) {
            log::warn!(
                "Heap page already mapped: {:#x}",
                page.start_address().as_u64()
            );
            continue;
        }

        let frame = frame_allocator
            .allocate_frame()
            .expect("Out of physical frames for heap");

        unsafe {
            mapper
                .map_to(
                    page,
                    frame,
                    PageTableFlags::PRESENT | PageTableFlags::WRITABLE,
                    frame_allocator,
                )
                .expect("Heap map_to failed")
                .flush();
        }

        log::trace!(
            "Mapped heap page: {:#x} → frame: {:#x}",
            page.start_address().as_u64(),
            frame.start_address().as_u64()
        );
    }

    unsafe {
        ALLOCATOR.lock().init(HEAP_START as *mut u8, HEAP_SIZE);
    }

    log::info!(
        "Heap initialized from {:#x} to {:#x}",
        HEAP_START,
        HEAP_START + HEAP_SIZE as u64
    );
}
