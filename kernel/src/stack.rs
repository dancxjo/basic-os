use x86_64::{
    VirtAddr,
    structures::paging::{FrameAllocator, Mapper, OffsetPageTable, Page, PageTableFlags},
};

use crate::allocator::BootFrameAllocator;

/// A 20 KB stack (5 pages)
#[repr(C, align(16))]
pub struct KernelStack([u8; 4096 * 5]);

pub static mut KERNEL_STACK: KernelStack = KernelStack([0; 4096 * 5]);

const KERNEL_STACK_VIRT_BASE: u64 = 0xffff_8800_0000_0000;
const KERNEL_STACK_PAGES: usize = 5;

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
