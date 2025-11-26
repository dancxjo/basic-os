use linked_list_allocator::LockedHeap;

#[global_allocator]
static ALLOCATOR: LockedHeap = LockedHeap::empty();
static mut HEAP_SPACE: [u8; 8 * 1024 * 1024] = [0; 8 * 1024 * 1024];

pub fn init_heap() {
    unsafe {
        ALLOCATOR
            .lock()
            .init(HEAP_SPACE.as_mut_ptr(), HEAP_SPACE.len());
    }
}
