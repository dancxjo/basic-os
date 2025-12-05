use core::ops::Range;
use core::sync::atomic::{AtomicBool, Ordering};
use spin::Mutex;
use x86_64::{
    PhysAddr, VirtAddr,
    registers::control::Cr3,
    structures::paging::{
        FrameAllocator, Mapper, OffsetPageTable, Page, PageTable, PageTableFlags, PhysFrame,
        Size4KiB, Translate,
    },
};

use crate::arch::x86_64::memory::{kernel_base, kernel_end};
#[cfg(not(feature = "mm_advanced"))]
use crate::bootloader::{collect_memory_regions, get_hhdm_offset};
#[cfg(feature = "debug_heap_bump")]
use crate::mm::bump_allocator::BumpAllocator;
#[cfg(all(not(feature = "debug_heap_bump"), feature = "debug_heap_canaries"))]
use crate::mm::debug_alloc::DebugAlloc;
#[cfg(feature = "mm_advanced")]
use crate::mm::pools::{MemoryRole, POOL_MANAGER, init_pools};
#[cfg(not(feature = "debug_heap_bump"))]
use linked_list_allocator::LockedHeap;

// Constants for heap placement

pub const HEAP_START: u64 = 0xFFFF_A000_0000_0000;
pub const HEAP_SIZE: usize = 32 * 1024 * 1024;

const SANITY_FORBIDDEN_RANGE: Range<u64> = 0x0010_0000..0x0100_0000;
const SANITY_DUMP_COUNT: usize = 8;

struct SanityCapture {
    frames: [Option<PhysAddr>; SANITY_DUMP_COUNT],
    count: usize,
    logged: bool,
}

impl SanityCapture {
    const fn new() -> Self {
        Self {
            frames: [None; SANITY_DUMP_COUNT],
            count: 0,
            logged: false,
        }
    }

    fn record(&mut self, addr: PhysAddr) {
        if self.count >= SANITY_DUMP_COUNT {
            return;
        }
        self.frames[self.count] = Some(addr);
        self.count += 1;
        if self.count == SANITY_DUMP_COUNT && !self.logged {
            self.logged = true;
            log_sanity_snapshot(&self.frames);
        }
    }
}

static SANITY_CAPTURE: Mutex<SanityCapture> = Mutex::new(SanityCapture::new());

#[derive(Clone)]
struct SanityBounds {
    kernel_image: Option<Range<u64>>,
    cr3_frame: u64,
}

static SANITY_BOUNDS: Mutex<Option<SanityBounds>> = Mutex::new(None);

#[cfg(feature = "debug_heap_bump")]
#[global_allocator]
static ALLOCATOR: BumpAllocator = BumpAllocator::new();

#[cfg(all(not(feature = "debug_heap_bump"), feature = "debug_heap_canaries"))]
#[global_allocator]
static ALLOCATOR: DebugAlloc<LockedHeap> = DebugAlloc::new(LockedHeap::empty());

#[cfg(all(not(feature = "debug_heap_bump"), not(feature = "debug_heap_canaries")))]
#[global_allocator]
static ALLOCATOR: LockedHeap = LockedHeap::empty();

// pub(crate) static mut MAPPER: MaybeUninit<OffsetPageTable> = MaybeUninit::uninit();
static ALLOCATOR_LOGGING_SAFE: AtomicBool = AtomicBool::new(false);

/// Initialize paging and return the active OffsetPageTable.
///
/// # Safety
///
/// This function is unsafe because the caller must guarantee that the
/// complete physical memory is mapped to virtual memory at the passed
/// `physical_memory_offset`.
pub unsafe fn init_paging(physical_memory_offset: VirtAddr) -> OffsetPageTable<'static> {
    let l4_table = unsafe { active_level_4_table(physical_memory_offset) };
    unsafe { OffsetPageTable::new(l4_table, physical_memory_offset) }
}

/// Returns a mutable reference to the active level 4 page table.
///
/// # Safety
///
/// This function is unsafe because the caller must guarantee that the
/// complete physical memory is mapped to virtual memory at the passed
/// `physical_memory_offset`. Also, this function must be only called once
/// or from a single thread to avoid aliasing `&mut` references (which is undefined behavior).
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
    #[cfg(not(feature = "mm_advanced"))]
    pub fn new() -> Self {
        let mut usable_ranges: [Option<Range<usize>>; 32] = Default::default();
        let mut range_count = 0;

        // Calculate physical stack range to exclude
        let rsp: u64;
        unsafe { core::arch::asm!("mov {}, rsp", out(reg) rsp) };
        let hhdm = get_hhdm_offset().as_u64();
        let phys_rsp = rsp.checked_sub(hhdm).expect("RSP below HHDM offset?");

        // Reserve 128KB below RSP and 4KB above (stack grows down)
        // Align to page boundaries
        let stack_phys_start = (phys_rsp.saturating_sub(0x20000)) & !0xFFF;
        let stack_phys_end = (phys_rsp + 0x1000 + 0xFFF) & !0xFFF;

        log::info!(
            "BootFrameAllocator: Reserving stack {:#x} - {:#x}",
            stack_phys_start,
            stack_phys_end
        );

        // FORCE RESERVE the observed stack collision range
        let collision_start = 0x37a5000;
        let collision_end = 0x37e6000;
        log::warn!(
            "BootFrameAllocator: FORCE RESERVING collision range {:#x} - {:#x}",
            collision_start,
            collision_end
        );

        for region in collect_memory_regions().iter() {
            if region.kind != "usable" {
                continue;
            }

            let region_start = core::cmp::max(region.base, SANITY_FORBIDDEN_RANGE.end);
            let region_end = region.base + region.len;

            if region_start >= region_end {
                continue;
            }

            // We need to exclude BOTH the boot stack AND the collision range.
            // This is getting complicated to do with simple splits.
            // Instead, let's just add the ranges that are valid.

            // Helper to add a range if it's valid
            let mut add_range = |start: u64, end: u64| {
                if start < end {
                    if range_count < usable_ranges.len() {
                        usable_ranges[range_count] = Some(start as usize..end as usize);
                        range_count += 1;
                    }
                }
            };

            // We have potentially 2 holes: Boot Stack and Collision Range.
            // Let's sort them.
            let mut holes = [
                (stack_phys_start, stack_phys_end),
                (collision_start, collision_end),
            ];
            holes.sort_by_key(|h| h.0);

            let mut current = region_start;
            for (hole_start, hole_end) in holes.iter() {
                // Add segment before hole
                let seg_end = core::cmp::min(region_end, *hole_start);
                add_range(current, seg_end);

                // Advance current past hole
                current = core::cmp::max(current, *hole_end);
            }
            // Add remaining segment
            add_range(current, region_end);
        }

        for i in 0..range_count {
            if let Some(r) = &usable_ranges[i] {
                log::info!("Range {}: {:#x} - {:#x}", i, r.start, r.end);
            }
        }

        assert!(range_count > 0, "BootFrameAllocator found no usable ranges");

        let first_range_start = usable_ranges[0].as_ref().unwrap().start;

        log::info!(
            "BootFrameAllocator (simple) initialized with {} usable ranges",
            range_count
        );

        Self {
            usable_ranges,
            range_count,
            current_range: 0,
            next: first_range_start,
        }
    }

    #[cfg(feature = "mm_advanced")]
    pub fn new() -> Self {
        init_pools();
        let pm = POOL_MANAGER.lock();

        let mut usable_ranges: [Option<Range<usize>>; 32] = Default::default();
        let mut range_count = 0;

        for pool in pm.pools[..pm.count].iter() {
            if pool.role == MemoryRole::GeneralFrames {
                if range_count >= 32 {
                    break;
                }
                usable_ranges[range_count] = Some(pool.start as usize..pool.end as usize);
                range_count += 1;
            }
        }

        assert!(
            range_count > 0,
            "No usable memory regions found for GeneralFrames!"
        );

        #[cfg(feature = "debug_frame_sanity")]
        {
            let bounds = get_sanity_bounds();
            for range in usable_ranges.iter().flatten() {
                let start = range.start as u64;
                let end = range.end as u64;
                assert!(
                    end <= SANITY_FORBIDDEN_RANGE.start || start >= SANITY_FORBIDDEN_RANGE.end,
                    "BootFrameAllocator usable range [{:#x}, {:#x}) overlaps forbidden window [{:#x}, {:#x})",
                    start,
                    end,
                    SANITY_FORBIDDEN_RANGE.start,
                    SANITY_FORBIDDEN_RANGE.end
                );
                if let Some(b) = bounds.as_ref() {
                    if let Some(kernel) = b.kernel_image.as_ref() {
                        assert!(
                            end <= kernel.start || start >= kernel.end,
                            "BootFrameAllocator usable range [{:#x}, {:#x}) overlaps kernel image [{:#x}, {:#x})",
                            start,
                            end,
                            kernel.start,
                            kernel.end
                        );
                    }
                    assert!(
                        b.cr3_frame < start || b.cr3_frame >= end,
                        "BootFrameAllocator usable range [{:#x}, {:#x}) overlaps active CR3 frame {:#x}",
                        start,
                        end,
                        b.cr3_frame
                    );
                }
            }
        }

        usable_ranges[..range_count].sort_by_key(|range| {
            usize::MAX - (range.as_ref().unwrap().end - range.as_ref().unwrap().start)
        });

        let first_range_start = usable_ranges[0].as_ref().unwrap().start;

        log::info!(
            "BootFrameAllocator initialized with {} usable ranges",
            range_count
        );
        for i in 0..range_count {
            let r = usable_ranges[i].as_ref().unwrap();
            log::info!("  Range {}: {:#x} - {:#x}", i, r.start, r.end);
        }

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
                #[cfg(feature = "debug_frame_sanity")]
                if let Some(reason) = forbidden_reason(aligned as u64) {
                    panic!(
                        "BootFrameAllocator allocated forbidden frame {:#x}: {}",
                        aligned, reason
                    );
                }

                self.next = aligned + 0x1000;
                // log::info!("Allocated {:#x}, next={:#x}", aligned, self.next);
                if aligned == 0x37a5000 {
                    log::warn!("Allocating 0x37a5000! next was {:#x}", aligned);
                }
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
}

unsafe impl FrameAllocator<Size4KiB> for BootFrameAllocator {
    fn allocate_frame(&mut self) -> Option<PhysFrame> {
        let addr = self.allocate_frame_internal()?;
        let phys_addr = PhysAddr::new(addr as u64);

        if addr >= 0x37a5000 && addr <= 0x37e6000 {
            log::warn!("Allocating frame in STACK RANGE: {:#x}", addr);
        }

        check_reserved_ranges(phys_addr);
        SANITY_CAPTURE.lock().record(phys_addr);
        let frame = PhysFrame::from_start_address(phys_addr).ok();
        if allocator_logging_enabled() {
            if let Some(f) = frame {
                log::trace!("Allocated frame: {:#x}", f.start_address().as_u64());
            }
        }
        frame
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
        #[cfg(all(not(feature = "debug_heap_bump"), not(feature = "debug_heap_canaries")))]
        ALLOCATOR.lock().init(HEAP_START as *mut u8, HEAP_SIZE);

        #[cfg(all(not(feature = "debug_heap_bump"), feature = "debug_heap_canaries"))]
        ALLOCATOR
            .inner()
            .lock()
            .init(HEAP_START as *mut u8, HEAP_SIZE);

        #[cfg(feature = "debug_heap_bump")]
        ALLOCATOR.init(HEAP_START as usize, HEAP_SIZE);
    }

    log::info!(
        "Heap initialized from {:#x} to {:#x}",
        HEAP_START,
        HEAP_START + HEAP_SIZE as u64
    );

    enable_allocator_logging();
}

fn allocator_logging_enabled() -> bool {
    ALLOCATOR_LOGGING_SAFE.load(Ordering::Relaxed)
}

fn enable_allocator_logging() {
    ALLOCATOR_LOGGING_SAFE.store(true, Ordering::SeqCst);
}

pub fn prime_allocator_sanity(mapper: &mut OffsetPageTable<'static>) {
    let kernel_range = translate_range(mapper, kernel_base(), kernel_end());
    let bounds = SanityBounds {
        kernel_image: kernel_range.clone(),
        cr3_frame: Cr3::read().0.start_address().as_u64(),
    };
    {
        let mut guard = SANITY_BOUNDS.lock();
        *guard = Some(bounds.clone());
    }

    if let Some(range) = bounds.kernel_image.as_ref() {
        log::info!(
            "Allocator sanity: kernel phys range {:#x} - {:#x}",
            range.start,
            range.end
        );
    } else {
        log::warn!("Allocator sanity: kernel phys range unavailable");
    }
    log::info!(
        "Allocator sanity: active CR3 frame at {:#x}",
        bounds.cr3_frame
    );
}

fn get_sanity_bounds() -> Option<SanityBounds> {
    SANITY_BOUNDS.lock().clone()
}

#[cfg(feature = "debug_frame_sanity")]
fn forbidden_reason(addr: u64) -> Option<&'static str> {
    if SANITY_FORBIDDEN_RANGE.contains(&addr) {
        return Some("forbidden physical window");
    }

    let Some(bounds) = get_sanity_bounds() else {
        return None;
    };

    if let Some(range) = bounds.kernel_image {
        if range.contains(&addr) {
            return Some("kernel image");
        }
    }

    if addr == bounds.cr3_frame {
        return Some("active CR3 frame");
    }

    None
}

fn check_reserved_ranges(addr: PhysAddr) {
    let value = addr.as_u64();
    #[cfg(feature = "debug_frame_sanity")]
    if let Some(reason) = forbidden_reason(value) {
        panic!(
            "Allocator returned forbidden frame {:#x}: {}",
            value, reason
        );
    }

    if SANITY_FORBIDDEN_RANGE.contains(&value) {
        panic!(
            "Allocated frame inside forbidden range: {:#x} - {:#x} (got {:#x})",
            SANITY_FORBIDDEN_RANGE.start, SANITY_FORBIDDEN_RANGE.end, value
        );
    }

    let Some(bounds) = get_sanity_bounds() else {
        return;
    };
    let SanityBounds {
        kernel_image,
        cr3_frame,
    } = bounds;

    if let Some(range) = kernel_image {
        if range.contains(&value) {
            panic!(
                "Allocator returned kernel image frame {:#x} (range {:#x} - {:#x})",
                value, range.start, range.end
            );
        }
    }

    if value == cr3_frame {
        panic!("Allocator returned active PML4 frame at {:#x}", cr3_frame);
    }
}

fn log_sanity_snapshot(frames: &[Option<PhysAddr>; SANITY_DUMP_COUNT]) {
    let bounds = get_sanity_bounds();
    log::warn!("==== Boot frame allocator sanity snapshot ====");
    log::warn!(
        "Forbidden physical window: {:#x} - {:#x}",
        SANITY_FORBIDDEN_RANGE.start,
        SANITY_FORBIDDEN_RANGE.end
    );
    match bounds.as_ref().and_then(|b| b.kernel_image.as_ref()) {
        Some(range) => log::warn!(
            "Kernel image physical range: {:#x} - {:#x}",
            range.start,
            range.end
        ),
        None => log::warn!("Kernel image physical range: unavailable"),
    }
    if let Some(b) = bounds.as_ref() {
        log::warn!("Active CR3 frame: {:#x}", b.cr3_frame);
    } else {
        log::warn!("Active CR3 frame: sanity bounds not initialized");
    }

    for (idx, entry) in frames.iter().enumerate() {
        match entry {
            Some(addr) => {
                let value = addr.as_u64();
                log::warn!("  frame[{}] = {:#x}", idx, value);
                if let Some(range) = bounds.as_ref().and_then(|b| b.kernel_image.as_ref()) {
                    if range.contains(&value) {
                        log::error!("    ↳ overlaps kernel image!");
                    }
                }
                if let Some(b) = bounds.as_ref() {
                    if value == b.cr3_frame {
                        log::error!("    ↳ overlaps active CR3 frame!");
                    }
                }
                if SANITY_FORBIDDEN_RANGE.contains(&value) {
                    log::error!("    ↳ inside forbidden physical window!");
                }
            }
            None => log::warn!("  frame[{}] = <unused>", idx),
        }
    }
}

fn translate_range(
    mapper: &mut OffsetPageTable<'static>,
    start: VirtAddr,
    end: VirtAddr,
) -> Option<Range<u64>> {
    if start >= end {
        return None;
    }
    let start_phys = mapper.translate_addr(start)?;
    let end_minus_one = VirtAddr::new(end.as_u64().saturating_sub(1));
    let end_phys = mapper.translate_addr(end_minus_one)?;
    Some(start_phys.as_u64()..(end_phys.as_u64() + 1))
}
