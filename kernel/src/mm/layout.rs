//! Virtual Memory Layout
//!
//! This module defines the virtual memory layout for the kernel and user space.
//!
//! # Kernel Space (Higher Half)
//!
//! The kernel is mapped in the higher half of the virtual address space.
//!
//! - `0xFFFF_8000_0000_0000`: Kernel Base (Code, Data, BSS)
//! - `0xFFFF_A000_0000_0000`: Kernel Heap
//! - `0xFFFF_8800_0000_0000`: Kernel Stacks (for tasks)
//!
//! # User Space (Lower Half)
//!
//! User applications run in the lower half of the virtual address space.
//!
//! - `0x0000_4000_0000_0000`: User Code (ELF Load Base for PIE/DYN)
//! - `0x0000_6000_0000_0000`: User Heap (Future use)
//! - `0x0000_7000_0000_0000`: User Stack Top (grows down)
//!

use x86_64::VirtAddr;

// --- Kernel Space ---

/// Base address for the kernel heap.
pub const KERNEL_HEAP_START: u64 = 0xFFFF_A000_0000_0000;
/// Size of the kernel heap.
pub const KERNEL_HEAP_SIZE: usize = 32 * 1024 * 1024; // 32 MiB

/// Base address for kernel task stacks.
pub const KERNEL_STACK_REGION_BASE: u64 = 0xFFFF_8800_1000_0000;
/// Maximum number of tasks to map stacks for (to avoid exhaustion).
pub const MAX_TASKS_TO_MAP: u64 = 64;

// --- User Space ---

/// Base address for loading user ELF executables (if position-independent).
pub const USER_CODE_BASE: u64 = 0x0000_4000_0000_0000;

/// Top of the user stack. The stack grows downwards from here.
pub const USER_STACK_TOP: u64 = 0x0000_7000_0000_0000;

/// Size of the user stack.
pub const USER_STACK_SIZE: usize = 16 * 4096; // 64 KiB

/// Base address for the user heap (future use).
pub const USER_HEAP_BASE: u64 = 0x0000_6000_0000_0000;

/// Helper to get the user stack range as a virtual address range.
pub fn user_stack_range() -> core::ops::Range<VirtAddr> {
    let top = VirtAddr::new(USER_STACK_TOP);
    let bottom = top - USER_STACK_SIZE as u64;
    bottom..top
}
