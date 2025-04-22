use crate::{bootloader::MEMMAP_REQUEST, serial_println};
use alloc::boxed::Box;
use core::{mem::MaybeUninit, ops::Range};
use limine::memory_map::EntryType;
use linked_list_allocator::LockedHeap;
use x86_64::{
    PhysAddr, VirtAddr,
    registers::control::Cr3,
    structures::paging::mapper::MapToError,
    structures::paging::{
        FrameAllocator, Mapper, OffsetPageTable, Page, PageTable, PageTableFlags, PhysFrame,
        Size4KiB,
    },
    structures::paging::{Size1GiB, Size2MiB},
};

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

/// High-level description of a memory region
#[derive(Clone, Copy, Debug)]
pub struct MemoryRegion {
    pub base: u64,
    pub len: u64,
    pub kind: &'static str,
}

/// Parse and cache Limine’s memory map
pub fn collect_memory_regions() -> &'static [MemoryRegion] {
    static mut CACHE: Option<&'static [MemoryRegion]> = None;
    static mut BUFF: [MemoryRegion; 128] = [MemoryRegion {
        base: 0,
        len: 0,
        kind: "unknown",
    }; 128];
    unsafe {
        if let Some(r) = CACHE {
            return r;
        }
        let resp = MEMMAP_REQUEST
            .get_response()
            .expect("No memory map from Limine");
        let mut count = 0;
        for e in resp.entries().iter() {
            let kind = match e.entry_type {
                EntryType::USABLE => "usable",
                EntryType::RESERVED => "reserved",
                EntryType::ACPI_RECLAIMABLE => "acpi_reclaimable",
                EntryType::ACPI_NVS => "acpi_nvs",
                EntryType::BAD_MEMORY => "bad_memory",
                EntryType::BOOTLOADER_RECLAIMABLE => "bootloader_reclaimable",
                EntryType::FRAMEBUFFER => "framebuffer",
                _ => "unknown",
            };
            BUFF[count] = MemoryRegion {
                base: e.base,
                len: e.length,
                kind,
            };
            serial_println!(
                "[Debug] region {}: base={:#x} len={:#x} kind={}",
                count,
                e.base,
                e.length,
                kind
            );
            count += 1;
        }
        let slice = &BUFF[..count];
        CACHE = Some(slice);
        serial_println!("[Debug] total regions = {}", count);
        slice
    }
}

/// Helper: map a 4KiB page, splitting any parent huge page if needed
pub fn map_page_to<M, F>(
    mapper: &mut M,
    page: Page<Size4KiB>,
    frame: PhysFrame,
    flags: PageTableFlags,
    allocator: &mut F,
) where
    M: Mapper<Size4KiB> + Mapper<Size2MiB> + Mapper<Size1GiB>,
    F: FrameAllocator<Size4KiB>,
{
    match unsafe { mapper.map_to(page, frame, flags, allocator) } {
        Ok(flush) => flush.flush(),
        Err(MapToError::ParentEntryHugePage) => {
            serial_println!(
                "[Warning] splitting huge page at virt={:#x}",
                page.start_address().as_u64()
            );
            let addr = page.start_address();
            // Try unmap 2MiB
            let p2 = Page::<Size2MiB>::containing_address(addr);
            if let Ok((_, flush2m)) = mapper.unmap(p2) {
                serial_println!(
                    "[Debug] unmap 2MiB huge page at {:#x}",
                    p2.start_address().as_u64()
                );
                flush2m.flush();
            } else {
                // Fallback unmap 1GiB
                let p1 = Page::<Size1GiB>::containing_address(addr);
                let (_, flush1g) = mapper.unmap(p1).expect("failed to unmap 1GiB huge page");
                serial_println!(
                    "[Debug] unmap 1GiB huge page at {:#x}",
                    p1.start_address().as_u64()
                );
                flush1g.flush();
            }
            // Retry
            let flush2 = unsafe { mapper.map_to(page, frame, flags, allocator) }
                .expect("map_to failed after split");
            flush2.flush();
        }
        Err(e) => panic!("map_page_to failed: {:?}", e),
    }
}

/// Map heap region with 4KiB pages
fn map_heap<M, F>(mapper: &mut M, frame_allocator: &mut F)
where
    M: Mapper<Size4KiB> + Mapper<Size2MiB> + Mapper<Size1GiB>,
    F: FrameAllocator<Size4KiB>,
{
    let start = VirtAddr::new(HEAP_START);
    let end = VirtAddr::new(start.as_u64() + HEAP_SIZE as u64);
    let flags = PageTableFlags::PRESENT | PageTableFlags::WRITABLE;
    serial_println!(
        "[Debug] map_heap: pages {:#x}-{:#x}",
        start.as_u64(),
        end.as_u64()
    );
    for page in Page::range_inclusive(
        Page::containing_address(start),
        Page::containing_address(VirtAddr::new(end.as_u64() - 1)),
    ) {
        let frame = frame_allocator.allocate_frame().expect("no frames");
        // serial_println!(
        //     "[Debug] page {:?} -> frame {:#x}",
        //     page,
        //     frame.start_address().as_u64()
        // );
        map_page_to(mapper, page, frame, flags, frame_allocator);
    }
    serial_println!("[Debug] map_heap complete");
}

/// Boot-time frame allocator (no heap)
pub struct BootFrameAllocator {
    ranges: &'static mut [Range<PhysAddr>],
    current_range: usize,
}

impl BootFrameAllocator {
    /// Create a new allocator from the provided memory regions
    pub fn new(regions: &[MemoryRegion]) -> Self {
        serial_println!("[Debug] BootFrameAllocator::new start");
        let mut count = 0;
        unsafe {
            for r in regions.iter() {
                if r.kind == "usable" && count < MAX_RANGES {
                    let start = align_up(r.base, 0x1000);
                    let end = (r.base + r.len) & !0xfff;
                    if end > start {
                        FRAME_RANGES[count].write(PhysAddr::new(start)..PhysAddr::new(end));
                        serial_println!("[Debug] adding range {}: {:#x}-{:#x}", count, start, end);
                        count += 1;
                    }
                }
            }
            #[allow(static_mut_refs)]
            let ranges = core::slice::from_raw_parts_mut(
                FRAME_RANGES.as_mut_ptr() as *mut Range<PhysAddr>,
                count,
            );
            serial_println!("[Debug] total usable ranges = {}", count);
            BootFrameAllocator {
                ranges,
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

/// Align up to multiple of `align`
fn align_up(addr: u64, align: u64) -> u64 {
    (addr + align - 1) & !(align - 1)
}

/// Initialize memory: paging, heap, frame allocator
pub unsafe fn init(
    hhdm_offset: VirtAddr,
) -> (&'static mut OffsetPageTable<'static>, BootFrameAllocator) {
    serial_println!("Initializing memory...");
    let lvl4 = unsafe { active_level_4_table(hhdm_offset) };
    let mut mapper = unsafe { OffsetPageTable::new(lvl4, hhdm_offset) };
    let regions = collect_memory_regions();
    let mut frame_allocator = BootFrameAllocator::new(regions);
    serial_println!(
        "[Debug] map heap {:#x}-{:#x}",
        HEAP_START,
        HEAP_START + HEAP_SIZE as u64
    );
    map_heap(&mut mapper, &mut frame_allocator);
    unsafe { ALLOCATOR.lock().init(HEAP_START as *mut u8, HEAP_SIZE) };
    (Box::leak(Box::new(mapper)), frame_allocator)
}

unsafe fn active_level_4_table(phys_offset: VirtAddr) -> &'static mut PageTable {
    let (frame, _) = Cr3::read();
    let virt = phys_offset + frame.start_address().as_u64();
    unsafe { &mut *(virt.as_mut_ptr()) }
}

/// Debug helper to print detected memory regions
pub fn print_memory_regions() {
    serial_println!("[Debug] print_memory_regions start");
    for (i, region) in collect_memory_regions().iter().enumerate() {
        serial_println!(
            "[Debug] region[{}] base={:#x} len={:#x} kind={}",
            i,
            region.base,
            region.len,
            region.kind
        );
    }
    serial_println!("[Debug] print_memory_regions end");
}
