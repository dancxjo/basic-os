use linked_list_allocator::LockedHeap;

// Define the heap size
const HEAP_SIZE: usize = 8 * 1024 * 1024; // 1 MiB heap

#[repr(align(16))]
struct AlignedHeap([u8; HEAP_SIZE]);

static mut HEAP_SPACE: AlignedHeap = AlignedHeap([0; HEAP_SIZE]);

#[global_allocator]
static ALLOCATOR: LockedHeap = LockedHeap::empty();

#[allow(static_mut_refs)]
pub fn init_heap() {
    unsafe {
        let heap_start = HEAP_SPACE.0.as_ptr() as *mut u8;
        ALLOCATOR.lock().init(heap_start, HEAP_SIZE);
    }
}
