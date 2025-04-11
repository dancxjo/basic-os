use linked_list_allocator::LockedHeap;
const HEAP_SIZE_IN_MIBS: usize = 4;
const HEAP_SIZE_IN_BYTES: usize = HEAP_SIZE_IN_MIBS * 1024 * 1024;
static mut HEAP: [u8; HEAP_SIZE_IN_BYTES] = [0; HEAP_SIZE_IN_BYTES];

#[global_allocator]
static ALLOCATOR: LockedHeap = LockedHeap::empty();

pub fn init_initial_allocator() {
    #[allow(static_mut_refs)]
    unsafe {
        ALLOCATOR.lock().init(HEAP.as_mut_ptr(), HEAP_SIZE_IN_BYTES);
    }
}
