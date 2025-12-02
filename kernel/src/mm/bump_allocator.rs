use core::alloc::{GlobalAlloc, Layout};
use core::ptr;
use core::sync::atomic::{AtomicUsize, Ordering};

/// Extremely simple bump allocator used for debugging.
///
/// Allocations bump a single pointer; deallocations are ignored.
pub struct BumpAllocator {
    start: AtomicUsize,
    size: AtomicUsize,
    next: AtomicUsize,
}

impl BumpAllocator {
    pub const fn new() -> Self {
        Self {
            start: AtomicUsize::new(0),
            size: AtomicUsize::new(0),
            next: AtomicUsize::new(0),
        }
    }

    /// Initialize the bump allocator with a heap window.
    pub unsafe fn init(&self, heap_start: usize, heap_size: usize) {
        self.start.store(heap_start, Ordering::SeqCst);
        self.size.store(heap_size, Ordering::SeqCst);
        self.next.store(heap_start, Ordering::SeqCst);
    }

    fn heap_end(&self) -> usize {
        let start = self.start.load(Ordering::SeqCst);
        start.saturating_add(self.size.load(Ordering::SeqCst))
    }
}

unsafe impl GlobalAlloc for BumpAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let align = layout.align().max(core::mem::size_of::<usize>());
        let size = layout.size().max(core::mem::size_of::<usize>());
        let heap_end = self.heap_end();

        let mut current = self.next.load(Ordering::SeqCst);
        loop {
            let aligned = (current + align - 1) & !(align - 1);
            let new_next = match aligned.checked_add(size) {
                Some(n) => n,
                None => return ptr::null_mut(),
            };
            if new_next > heap_end {
                return ptr::null_mut();
            }

            match self
                .next
                .compare_exchange(current, new_next, Ordering::SeqCst, Ordering::SeqCst)
            {
                Ok(_) => return aligned as *mut u8,
                Err(old) => current = old,
            }
        }
    }

    unsafe fn dealloc(&self, _ptr: *mut u8, _layout: Layout) {
        // Intentionally leak; bump allocators do not free.
    }
}
