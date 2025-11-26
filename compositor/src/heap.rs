use core::alloc::{GlobalAlloc, Layout};
use linked_list_allocator::LockedHeap;
use userland::println;

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
        let start = unsafe { HEAP_SPACE.0.as_ptr() as usize };
        let end = start + unsafe { HEAP_SPACE.0.len() };
        let addr = ptr as usize;
        addr >= start && addr < end
    }
}

unsafe impl GlobalAlloc for SafeHeap {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let ptr = self.inner.alloc(layout);
        println!("alloc size={} => {:p}", layout.size(), ptr);
        ptr
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        // Ignore null pointers and zero-sized deallocations: they don't
        // correspond to real heap blocks and can confuse the linked-list
        // allocator (leading to aliasing / double-free panics).
        if ptr.is_null() || layout.size() == 0 {
            return;
        }

        if self.in_heap(ptr) {
            println!("dealloc ptr={:p} size={}", ptr, layout.size());
            self.inner.dealloc(ptr, layout);
        } else {
             println!("dealloc ptr={:p} NOT IN HEAP", ptr);
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

#[repr(C, align(4096))]
struct HeapBuffer([u8; 32 * 1024 * 1024]);

static mut HEAP_SPACE: HeapBuffer = HeapBuffer([0; 32 * 1024 * 1024]);

pub fn init_heap() {
    unsafe {
        ALLOCATOR
            .inner
            .lock()
            .init(HEAP_SPACE.0.as_mut_ptr(), HEAP_SPACE.0.len());
    }
}
