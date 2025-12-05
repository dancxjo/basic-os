use x86_64::{
    VirtAddr,
    structures::paging::{FrameAllocator, Mapper, OffsetPageTable, Page, PageTableFlags},
};

use crate::mm::allocator::BootFrameAllocator;

pub const KERNEL_STACK_PAGES: usize = 16;
const KERNEL_STACK_SIZE: usize = 4096 * KERNEL_STACK_PAGES;

/// Kernel stack storage.
#[repr(C, align(16))]
pub struct KernelStack([u8; KERNEL_STACK_SIZE]);

pub static mut KERNEL_STACK: KernelStack = KernelStack([0; KERNEL_STACK_SIZE]);

pub const KERNEL_STACK_VIRT_BASE: u64 = 0xffff_8800_0000_0000;

pub static mut KERNEL_STACK_TOP: VirtAddr = VirtAddr::zero();
/// Allocate and map a kernel stack at a fresh virtual address
pub unsafe fn init_kernel_stack(
    mapper: &mut OffsetPageTable,
    frame_allocator: &mut BootFrameAllocator,
) {
    let stack_start = VirtAddr::new(KERNEL_STACK_VIRT_BASE);
    let mut page = Page::containing_address(stack_start);

    for _ in 0..KERNEL_STACK_PAGES {
        let frame = frame_allocator
            .allocate_frame()
            .expect("Out of physical frames!");

        unsafe {
            mapper
                .map_to(
                    page,
                    frame,
                    PageTableFlags::PRESENT | PageTableFlags::WRITABLE,
                    frame_allocator,
                )
                .expect("map_to failed (stack)")
                .flush()
        };

        page = page + 1;
    }

    unsafe { KERNEL_STACK_TOP = stack_start + (KERNEL_STACK_PAGES as u64 * 4096) };
}

// TODO(STACK-GUARD): Implement guard pages for kernel stacks.
// This function is a sketch of how we might map a stack with a guard page.
// It should be used in place of the loop in init_kernel_stack once fully implemented.
#[allow(dead_code)]
fn map_kernel_stack_with_guard(
    mapper: &mut OffsetPageTable,
    frame_allocator: &mut BootFrameAllocator,
    start_page: Page,
    pages: usize,
) {
    // 1. Map guard page as non-present (or just don't map it).
    //    If we want to be explicit, we can map it with NO_ACCESS if the architecture supports it,
    //    or just leave it unmapped (which causes a page fault).
    //    For now, we assume the page below the stack is the guard page.

    // let guard_page = start_page;
    // log::info!("Guard page at {:?}", guard_page.start_address());

    // 2. Map remaining pages as RW.
    let stack_pages_start = start_page + 1;
    for i in 0..pages {
        let page = stack_pages_start + i as u64;
        let frame = frame_allocator
            .allocate_frame()
            .expect("Out of physical frames for stack");

        unsafe {
            mapper
                .map_to(
                    page,
                    frame,
                    PageTableFlags::PRESENT | PageTableFlags::WRITABLE,
                    frame_allocator,
                )
                .expect("map_to failed (stack)")
                .flush()
        };
    }
}
