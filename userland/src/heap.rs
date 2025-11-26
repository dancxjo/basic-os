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
}

unsafe impl GlobalAlloc for SafeHeap {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let mut heap = self.inner.lock();
        heap.allocate_first_fit(layout)
            .ok()
            .map_or(core::ptr::null_mut(), |allocation| allocation.as_ptr())
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        let mut heap = self.inner.lock();
        unsafe {
            heap.deallocate(core::ptr::NonNull::new_unchecked(ptr), layout);
        }
    }
}

#[global_allocator]
static ALLOCATOR: SafeHeap = SafeHeap::new();

#[repr(C, align(4096))]
struct HeapBuffer([u8; 32 * 1024 * 1024]);

static mut HEAP_SPACE: HeapBuffer = HeapBuffer([0; 32 * 1024 * 1024]);

pub fn init_heap() {
    unsafe {
        let start = HEAP_SPACE.0.as_mut_ptr();
        let len = HEAP_SPACE.0.len();
        ALLOCATOR.inner.lock().init(start, len);
    }
}
