use crate::bootloader::collect_memory_regions;
use spin::Mutex;
use x86_64::{
    PhysAddr,
    structures::paging::{FrameAllocator, PhysFrame, Size4KiB},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MemoryRole {
    PageTables,
    KernelStacks,
    GeneralFrames,
    Dma,
    Reserved,
    Unknown,
}

#[derive(Debug, Clone, Copy)]
pub struct MemoryPool {
    pub start: u64,
    pub end: u64,
    pub role: MemoryRole,
}

pub const MAX_POOLS: usize = 64;

pub struct PoolManager {
    pub pools: [MemoryPool; MAX_POOLS],
    pub count: usize,
    initialized: bool,
}

impl PoolManager {
    pub const fn new() -> Self {
        Self {
            pools: [MemoryPool {
                start: 0,
                end: 0,
                role: MemoryRole::Unknown,
            }; MAX_POOLS],
            count: 0,
            initialized: false,
        }
    }

    pub fn init(&mut self) {
        if self.initialized {
            return;
        }
        self.initialized = true;
        let regions = collect_memory_regions();

        // Simple strategy:
        // 1. Find the largest usable region.
        // 2. Carve out PageTables (4MB) and KernelStacks (4MB) from the start of it.
        // 3. The rest is GeneralFrames.
        // 4. All other usable regions are GeneralFrames.

        let mut largest_idx = 0;
        let mut largest_len = 0;

        for (i, r) in regions.iter().enumerate() {
            if r.kind == "usable" && r.len > largest_len {
                largest_len = r.len;
                largest_idx = i;
            }
        }

        // Define sizes
        let pt_size = 4 * 1024 * 1024; // 4 MiB
        let stack_size = 4 * 1024 * 1024; // 4 MiB
        let reserved_low = 0x100000; // 1 MiB to skip low memory

        for (i, r) in regions.iter().enumerate() {
            if r.kind != "usable" {
                continue;
            }

            let mut start = r.base;
            let end = r.base + r.len;

            // Skip low memory if this region starts at 0
            if start < reserved_low {
                start = reserved_low;
                if start >= end {
                    continue;
                }
            }

            if i == largest_idx {
                // Carve out pools
                // Ensure we have enough space
                if (end - start) < (pt_size + stack_size + 4096) {
                    // Fallback if largest region is too small?
                    // For now, just mark as GeneralFrames
                    self.add_pool(start, end, MemoryRole::GeneralFrames);
                    continue;
                }

                let pt_end = start + pt_size;
                self.add_pool(start, pt_end, MemoryRole::PageTables);

                let stack_end = pt_end + stack_size;
                self.add_pool(pt_end, stack_end, MemoryRole::KernelStacks);

                self.add_pool(stack_end, end, MemoryRole::GeneralFrames);
            } else {
                self.add_pool(start, end, MemoryRole::GeneralFrames);
            }
        }
    }

    fn add_pool(&mut self, start: u64, end: u64, role: MemoryRole) {
        if self.count < MAX_POOLS {
            self.pools[self.count] = MemoryPool { start, end, role };
            self.count += 1;
        }
    }

    pub fn get_pools_by_role(&self, role: MemoryRole) -> impl Iterator<Item = &MemoryPool> {
        self.pools[..self.count]
            .iter()
            .filter(move |p| p.role == role)
    }
}

pub static POOL_MANAGER: Mutex<PoolManager> = Mutex::new(PoolManager::new());

pub fn init_pools() {
    POOL_MANAGER.lock().init();
}

pub struct PageTableAllocator {
    next: u64,
}

pub static PAGE_TABLE_ALLOCATOR: Mutex<PageTableAllocator> = Mutex::new(PageTableAllocator::new());

impl PageTableAllocator {
    pub const fn new() -> Self {
        Self { next: 0 }
    }

    pub fn alloc_pt_frame(&mut self) -> Option<PhysFrame<Size4KiB>> {
        let mut pm = POOL_MANAGER.lock();

        // Find the PageTables pool
        // For simplicity, we assume there is only one or we iterate through them.
        // But we need to store state (next) per pool or globally.
        // If we have multiple pools for PageTables, we need to track where we are.

        // To keep it simple and stateless in the struct (except 'next'),
        // we can iterate pools and see if 'next' falls within one.
        // Or we can initialize 'next' to the start of the first PageTable pool.

        if self.next == 0 {
            // Initialize
            for pool in pm.pools[..pm.count].iter() {
                if pool.role == MemoryRole::PageTables {
                    self.next = pool.start;
                    break;
                }
            }
            if self.next == 0 {
                return None;
            } // No pool found
        }

        // Check if next is valid
        let mut current_pool_end = 0;
        let mut found = false;

        for pool in pm.pools[..pm.count].iter() {
            if pool.role == MemoryRole::PageTables {
                if self.next >= pool.start && self.next < pool.end {
                    current_pool_end = pool.end;
                    found = true;
                    break;
                }
            }
        }

        if !found {
            // Maybe we exhausted one pool and need to move to the next?
            // For now, let's assume one contiguous pool or fail.
            return None;
        }

        if self.next >= current_pool_end {
            return None;
        }

        let frame_addr = self.next;
        self.next += 4096;

        Some(PhysFrame::from_start_address(PhysAddr::new(frame_addr)).unwrap())
    }
}

unsafe impl FrameAllocator<Size4KiB> for PageTableAllocator {
    fn allocate_frame(&mut self) -> Option<PhysFrame<Size4KiB>> {
        self.alloc_pt_frame()
    }
}
