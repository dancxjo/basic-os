use linked_list_allocator::LockedHeap;

/// Global heap allocator used by Box, Vec, etc.
#[global_allocator]
pub static ALLOCATOR: LockedHeap = LockedHeap::empty();
