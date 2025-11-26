use core::ops::Range;

use x86_64::{
    VirtAddr,
    structures::paging::{FrameAllocator, Mapper, PageTableFlags, Size4KiB},
};

pub fn mirror_kernel_region(
    mapper: &mut impl Mapper<Size4KiB>,
    frame_allocator: &mut impl FrameAllocator<Size4KiB>,
    virt_range: Range<VirtAddr>,
) {
    use x86_64::structures::paging::{Page, PhysFrame, Translate};

    for addr in (virt_range.start.as_u64()..virt_range.end.as_u64()).step_by(4096) {
        let va = VirtAddr::new(addr);
        let page = Page::containing_address(va);

        // Translate from current active mapper
        #[allow(static_mut_refs)]
        let active_mapper = unsafe { crate::mm::allocator::MAPPER.assume_init_mut() };

        if let Some(phys_addr) = active_mapper.translate_addr(va) {
            let frame = PhysFrame::containing_address(phys_addr);
            unsafe {
                match mapper.map_to(
                    page,
                    frame,
                    PageTableFlags::PRESENT | PageTableFlags::WRITABLE,
                    frame_allocator,
                ) {
                    Ok(flusher) => flusher.flush(),
                    Err(x86_64::structures::paging::mapper::MapToError::PageAlreadyMapped(_)) => {
                        // Ignore
                    }
                    Err(e) => panic!("failed to mirror kernel region: {:?}", e),
                }
            }
        }
    }
}
