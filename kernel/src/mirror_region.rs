use core::range::Range;

use x86_64::{
    VirtAddr,
    structures::paging::{FrameAllocator, Mapper, PageTableFlags, Size4KiB},
};

pub fn mirror_kernel_region(
    mapper: &mut impl Mapper<Size4KiB>,
    frame_allocator: &mut impl FrameAllocator<Size4KiB>,
    virt_range: Range<VirtAddr>,
) {
    use x86_64::structures::paging::Page;

    for addr in (virt_range.start.as_u64()..virt_range.end.as_u64()).step_by(4096) {
        let va = VirtAddr::new(addr);
        let page = Page::containing_address(va);

        // Translate from current active mapper
        #[allow(static_mut_refs)]
        if let Ok(frame) =
            unsafe { crate::allocator::MAPPER.assume_init_mut() }.translate_page(page)
        {
            unsafe {
                mapper
                    .map_to(
                        page,
                        frame,
                        PageTableFlags::PRESENT | PageTableFlags::WRITABLE,
                        frame_allocator,
                    )
                    .expect("failed to mirror kernel region")
                    .flush();
            }
        }
    }
}
