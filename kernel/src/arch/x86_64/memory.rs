use x86_64::VirtAddr;

unsafe extern "C" {
    /// Start of the kernel image in virtual memory.
    static KERNEL_BASE: u8;
    /// End of the kernel image in virtual memory.
    static KERNEL_END: u8;
}

/// Virtual address of the first byte of the kernel image.
pub fn kernel_base() -> VirtAddr {
    unsafe { VirtAddr::from_ptr(&KERNEL_BASE) }
}

/// Virtual address past the last byte of the kernel image.
pub fn kernel_end() -> VirtAddr {
    unsafe { VirtAddr::from_ptr(&KERNEL_END) }
}

/// Check if a kernel address is mapped in the given CR3.
pub fn check_kernel_mapping(cr3: x86_64::PhysAddr, addr: VirtAddr) -> bool {
    use crate::bootloader::get_hhdm_offset;
    use x86_64::structures::paging::{OffsetPageTable, Translate};

    let hhdm = get_hhdm_offset();
    let l4_table = unsafe {
        let virt = hhdm + cr3.as_u64();
        &mut *virt.as_mut_ptr::<x86_64::structures::paging::PageTable>()
    };
    let mapper = unsafe { OffsetPageTable::new(l4_table, hhdm) };
    mapper.translate_addr(addr).is_some()
}
