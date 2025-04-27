use x86_64::structures::paging::{FrameAllocator, Mapper, PageTableFlags, Size4KiB};

pub const STACK_START: u64 = 0xffff8000007f8000; // Bottom of stack (lowest address)
pub const STACK_SIZE: usize = 32 * 1024; // 32 KiB
pub const STACK_TOP: u64 = STACK_START + STACK_SIZE as u64;

pub fn map_kernel_stack<M: Mapper<Size4KiB>>(
    mapper: &mut M,
    frame_allocator: &mut impl FrameAllocator<Size4KiB>,
) {
    use x86_64::VirtAddr;
    use x86_64::structures::paging::Page;

    let stack_start = VirtAddr::new(STACK_START);
    let stack_end = stack_start + STACK_SIZE as u64;

    for page in Page::range_inclusive(
        Page::containing_address(stack_start),
        Page::containing_address(stack_end - 1u64),
    ) {
        let frame = frame_allocator
            .allocate_frame()
            .expect("No frame for stack!");
        let flags = PageTableFlags::PRESENT | PageTableFlags::WRITABLE;

        unsafe {
            mapper
                .map_to(page, frame, flags, frame_allocator)
                .expect("stack map_to failed")
                .flush();
        }
    }

    log::info!("Kernel stack mapped.");
}
