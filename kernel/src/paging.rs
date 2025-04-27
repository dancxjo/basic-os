//! paging.rs — Fresh paging setup for ThingOS, Limine-aware

use crate::bootloader::get_hhdm_offset;
use crate::serial_println;
use crate::stack::map_kernel_stack;
use crate::{allocator::ALLOCATOR, bootloader::MEMMAP_REQUEST};
use core::mem::MaybeUninit;
use core::range::Range;
use limine::{memory_map::EntryType, request::MemoryMapRequest};
use x86_64::registers::debug;
use x86_64::structures::paging::page;
use x86_64::{
    PhysAddr, VirtAddr,
    registers::control::Cr3,
    structures::paging::{
        FrameAllocator, Mapper, OffsetPageTable, Page, PageTable, PageTableFlags, PhysFrame,
        Size4KiB,
    },
};

pub const HEAP_START: u64 = 0x4444_4444_0000;
pub const HEAP_SIZE: usize = 16 * 1024 * 1024; // 16 MiB

static mut MAPPER: MaybeUninit<OffsetPageTable> = MaybeUninit::uninit();

pub struct BootFrameAllocator {
    usable_ranges: [Option<core::range::Range<usize>>; 32],
    range_count: usize,
    current_range: usize,
    next: usize,
}

static mut INSTANCE: Option<BootFrameAllocator> = None;

impl BootFrameAllocator {
    pub fn init() -> &'static mut Self {
        unsafe {
            #[allow(static_mut_refs)]
            INSTANCE.get_or_insert_with(|| {
                let regions = collect_memory_regions();
                let mut usable_ranges: [Option<Range<usize>>; 32] = [None; 32];
                let mut range_count = 0;

                for r in regions
                    .iter()
                    .filter(|r| r.kind == "usable" && r.len >= 0x200000)
                {
                    if range_count >= usable_ranges.len() {
                        break;
                    }
                    let range = r.base as usize..(r.base + r.len) as usize;
                    usable_ranges[range_count] = Some(range.into());
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
}

unsafe impl FrameAllocator<Size4KiB> for BootFrameAllocator {
    fn allocate_frame(&mut self) -> Option<PhysFrame> {
        let addr = self.allocate_frame()?;
        let frame = PhysFrame::from_start_address(PhysAddr::new(addr as u64)).unwrap();
        Some(frame)
    }
}

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

pub unsafe fn init_paging(
    frame_allocator: &mut impl FrameAllocator<Size4KiB>,
) -> OffsetPageTable<'static> {
    log::debug!("Initializing paging...");

    let frame = frame_allocator
        .allocate_frame()
        .expect("No frame for new PML4");

    log::info!("Allocated frame for new PML4");

    let phys = frame.start_address().as_u64();
    let virt = VirtAddr::new(phys + get_hhdm_offset().as_u64());
    let pml4_ptr = virt.as_u64() as *mut PageTable;
    unsafe { pml4_ptr.write(PageTable::new()) };

    log::info!("New PML4 page table initialized at: {:#x}", virt.as_u64());
    let mut mapper =
        unsafe { OffsetPageTable::new(&mut *pml4_ptr, VirtAddr::new(get_hhdm_offset().as_u64())) };

    map_kernel_stack(&mut mapper, frame_allocator);
    log::info!("Kernel stack mapped. Mapping heap...");
    identity_map_kernel(&mut mapper, frame_allocator);
    log::info!("Kernel identity mapped. Mapping heap...");

    map_heap(&mut mapper, frame_allocator);
    log::info!("Heap mapped.");
    #[allow(unconditional_panic)]
    let fail = 1 / 0;
    loop {}

    unsafe { Cr3::write(frame, Cr3::read().1) };

    log::info!("Paging initialized and switched to new PML4.");

    mapper
}

fn map_heap<M: Mapper<Size4KiB>>(
    mapper: &mut M,
    frame_allocator: &mut impl FrameAllocator<Size4KiB>,
) {
    let heap_start = VirtAddr::new(HEAP_START);
    let heap_end = heap_start + HEAP_SIZE as u64;
    log::info!(
        "Mapping heap from {:#x} to {:#x}",
        heap_start.as_u64(),
        heap_end.as_u64()
    );

    for page in Page::range_inclusive(
        Page::containing_address(heap_start),
        Page::containing_address(heap_end - 1u64),
    ) {
        log::info!("Mapping page: {:#x}", page.start_address().as_u64());

        let frame = frame_allocator
            .allocate_frame()
            .expect("Frame allocation failed during heap mapping");
        let flags = PageTableFlags::PRESENT | PageTableFlags::WRITABLE;

        log::info!(
            "Allocated frame: {:#x} for page: {:#x}",
            frame.start_address().as_u64(),
            page.start_address().as_u64()
        );
        log::info!("Frame: {:#x}", frame.start_address().as_u64());
        log::info!("Page: {:#x}", page.start_address().as_u64());
        log::info!("Flags: {:#x}", flags.bits());
        log::info!("Frame allocator: {:#x}", frame_allocator as *const _ as u64);
        log::info!("Mapper: {:#x}", mapper as *const _ as u64);

        unsafe {
            mapper
                .map_to(page, frame, flags, frame_allocator)
                .expect("Heap map_to failed")
                .flush();
        }
        log::info!("Mapped page: {:#x}", page.start_address().as_u64());
    }
    log::info!("ALLOCATOR address: {:#x}", &ALLOCATOR as *const _ as usize);

    unsafe {
        ALLOCATOR.lock().init(HEAP_START as *mut u8, HEAP_SIZE);
    }

    log::info!("Heap allocator initialized.");
    log::info!("Heap start: {:#x}", HEAP_START);
    log::info!("Heap size: {:#x}", HEAP_SIZE);

    log::info!("Heap mapped and allocator initialized.");
}

const KERNEL_VIRT_BASE: u64 = 0xffffffff80000000;

pub fn identity_map_kernel(
    mapper: &mut OffsetPageTable,
    frame_allocator: &mut impl FrameAllocator<Size4KiB>,
) {
    let kernel_start = VirtAddr::new(KERNEL_VIRT_BASE);

    // 🚨 Map up to and including ALLOCATOR, with a safety buffer
    let kernel_end = VirtAddr::new(0xffffffff80030000); // Map more generously

    let start_page = Page::<Size4KiB>::containing_address(kernel_start);
    let end_page = Page::<Size4KiB>::containing_address(kernel_end);

    for page in Page::range_inclusive(start_page, end_page) {
        let virt_addr = page.start_address();
        let phys_addr = PhysAddr::new(virt_addr.as_u64() - get_hhdm_offset().as_u64());
        let frame = PhysFrame::containing_address(phys_addr);

        unsafe {
            mapper
                .map_to(
                    page,
                    frame,
                    PageTableFlags::PRESENT | PageTableFlags::WRITABLE,
                    frame_allocator,
                )
                .expect("Kernel identity mapping failed")
                .flush();
        }
    }

    log::info!(
        "Kernel identity mapped from {:#x} to {:#x}",
        kernel_start.as_u64(),
        kernel_end.as_u64()
    );
}
