#[cfg(all(
    not(test),
    not(feature = "std"),
    not(feature = "kernel_hosted"),
    not(feature = "kernel_standalone")
))]
use core::alloc::{GlobalAlloc, Layout};
#[cfg(all(
    not(test),
    not(feature = "std"),
    not(feature = "kernel_hosted"),
    not(feature = "kernel_standalone")
))]
use linked_list_allocator::LockedHeap;

#[cfg(test)]
extern crate std;

#[cfg(all(
    not(test),
    not(feature = "std"),
    not(feature = "kernel_hosted"),
    not(feature = "kernel_standalone")
))]
struct SafeHeap {
    inner: LockedHeap,
}

#[cfg(all(
    not(test),
    not(feature = "std"),
    not(feature = "kernel_hosted"),
    not(feature = "kernel_standalone")
))]
impl SafeHeap {
    pub const fn new() -> Self {
        Self {
            inner: LockedHeap::empty(),
        }
    }
}

#[cfg(all(
    not(test),
    not(feature = "std"),
    not(feature = "kernel_hosted"),
    not(feature = "kernel_standalone")
))]
unsafe impl GlobalAlloc for SafeHeap {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let mut heap = self.inner.lock();
        heap.allocate_first_fit(layout)
            .ok()
            .map_or(core::ptr::null_mut(), |allocation| allocation.as_ptr())
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        let mut heap = self.inner.lock();
        heap.deallocate(core::ptr::NonNull::new_unchecked(ptr), layout);
    }
}

#[cfg(all(
    not(test),
    not(feature = "std"),
    not(feature = "kernel_hosted"),
    not(feature = "kernel_standalone")
))]
#[global_allocator]
static ALLOCATOR: SafeHeap = SafeHeap::new();

#[cfg(test)]
#[global_allocator]
static ALLOCATOR: std::alloc::System = std::alloc::System;

#[cfg(all(
    not(test),
    not(feature = "std"),
    not(feature = "kernel_hosted"),
    not(feature = "kernel_standalone")
))]
#[repr(C, align(4096))]
struct HeapBuffer([u8; 32 * 1024 * 1024]);

#[cfg(all(
    not(test),
    not(feature = "std"),
    not(feature = "kernel_hosted"),
    not(feature = "kernel_standalone")
))]
static mut HEAP_SPACE: HeapBuffer = HeapBuffer([0; 32 * 1024 * 1024]);

#[cfg(all(
    not(test),
    not(feature = "std"),
    not(feature = "kernel_hosted"),
    not(feature = "kernel_standalone")
))]
pub fn init_heap() {
    unsafe {
        let start = HEAP_SPACE.0.as_mut_ptr();
        let len = HEAP_SPACE.0.len();
        crate::println!("Initializing user heap at {:p} with size {}", start, len);
        ALLOCATOR.inner.lock().init(start, len);
    }
}

#[cfg(any(
    test,
    feature = "std",
    feature = "kernel_hosted",
    feature = "kernel_standalone"
))]
pub fn init_heap() {}

#[cfg(all(
    not(test),
    not(feature = "std"),
    not(feature = "kernel_hosted"),
    not(feature = "kernel_standalone")
))]
#[alloc_error_handler]
fn alloc_error_handler(layout: Layout) -> ! {
    crate::println!("ALLOCATION FAILED: layout={:?}", layout);
    loop {}
}
