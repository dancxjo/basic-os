use core::alloc::{GlobalAlloc, Layout};
use linked_list_allocator::LockedHeap;

struct SafeHeap {
    inner: LockedHeap,
}

impl SafeHeap {
    pub const fn new() -> Self {
        Self {
            inner: LockedHeap::empty(),
        }
    }

    fn in_heap(&self, ptr: *mut u8) -> bool {
        // SAFETY: Reading bounds of the static buffer is safe because we never
        // mutate it here; the buffer is a fixed backing store.
        let start = unsafe { HEAP_SPACE.as_ptr() as usize };
        let end = start + unsafe { HEAP_SPACE.len() };
        let addr = ptr as usize;
        addr >= start && addr < end
    }
}

unsafe impl GlobalAlloc for SafeHeap {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        self.inner.alloc(layout)
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        if self.in_heap(ptr) {
            self.inner.dealloc(ptr, layout);
        }
    }

    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        if self.in_heap(ptr) {
            self.inner.realloc(ptr, layout, new_size)
        } else {
            core::ptr::null_mut()
        }
    }
}

#[global_allocator]
static ALLOCATOR: SafeHeap = SafeHeap::new();
static mut HEAP_SPACE: [u8; 8 * 1024 * 1024] = [0; 8 * 1024 * 1024];

pub fn init_heap() {
    unsafe {
        ALLOCATOR
            .inner
            .lock()
            .init(HEAP_SPACE.as_mut_ptr(), HEAP_SPACE.len());
    }
}
