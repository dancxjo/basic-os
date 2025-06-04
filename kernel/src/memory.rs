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
