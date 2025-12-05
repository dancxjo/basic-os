use core::alloc::{GlobalAlloc, Layout};
use core::ptr::{self, read_unaligned, write_unaligned};
use log;

const CANARY: u64 = 0xfeed_face_dead_beef;

/// Wraps another allocator and places canaries around each allocation.
pub struct DebugAlloc<A> {
    inner: A,
}

impl<A> DebugAlloc<A> {
    pub const fn new(inner: A) -> Self {
        Self { inner }
    }

    pub fn inner(&self) -> &A {
        &self.inner
    }
}

unsafe impl<A: GlobalAlloc> GlobalAlloc for DebugAlloc<A> {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let prefix = core::mem::size_of::<u64>().max(layout.align());
        let total_size = match layout
            .size()
            .checked_add(prefix)
            .and_then(|s| s.checked_add(core::mem::size_of::<u64>()))
        {
            Some(size) => size,
            None => return ptr::null_mut(),
        };
        let alloc_align = layout.align().max(core::mem::align_of::<u64>());
        let new_layout = match Layout::from_size_align(total_size, alloc_align) {
            Ok(layout) => layout,
            Err(_) => return ptr::null_mut(),
        };

        // log::trace!("[ALLOC] DebugAlloc: size={}, align={}", layout.size(), layout.align());
        log::trace!(
            "[ALLOC] DebugAlloc: size={}, align={}",
            layout.size(),
            layout.align()
        );

        let raw = self.inner.alloc(new_layout);
        if raw.is_null() {
            return ptr::null_mut();
        }

        let user_ptr = raw.add(prefix);
        let header = user_ptr.sub(core::mem::size_of::<u64>()) as *mut u64;
        write_unaligned(header, CANARY);

        let footer = user_ptr.add(layout.size()) as *mut u64;
        write_unaligned(footer, CANARY);
        user_ptr
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        if ptr.is_null() {
            return;
        }

        let prefix = core::mem::size_of::<u64>().max(layout.align());
        let header = ptr.sub(core::mem::size_of::<u64>()) as *mut u64;
        let footer = ptr.add(layout.size()) as *mut u64;

        let head = read_unaligned(header);
        let foot = read_unaligned(footer);

        if head != CANARY || foot != CANARY {
            panic!(
                "Heap canary corrupted: header={:#x} footer={:#x} (expected {:#x})",
                head, foot, CANARY
            );
        }

        let total_size = match layout
            .size()
            .checked_add(prefix)
            .and_then(|s| s.checked_add(core::mem::size_of::<u64>()))
        {
            Some(size) => size,
            None => return,
        };
        let alloc_align = layout.align().max(core::mem::align_of::<u64>());
        let new_layout = match Layout::from_size_align(total_size, alloc_align) {
            Ok(layout) => layout,
            Err(_) => return,
        };

        self.inner.dealloc(ptr.sub(prefix), new_layout);
    }
}
