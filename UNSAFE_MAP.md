# Unsafe Map – ThingOS Kernel

## Legend
- Categories: ARCH, MEMORY, SYNC, FFI, OTHER
- Risk: low / medium / high

## Entries

### Syscalls (`kernel/src/arch/x86_64/syscall.rs`)

- `syscall_entry`
  - Category: FFI
  - Risk: high
  - Description: Main entry point for syscalls. Dispatches based on syscall number.

- `USER_RSP`
  - Category: SYNC
  - Risk: high
  - Description: Global mutable static to save user stack pointer. Not thread-safe if multiple cores were used (currently single core).

- `get_self`, `graph_fiat`, `graph_link`, `watch_register`, `kbd_read`, `mouse_read`, `graph_get`, `graph_find_by_kind`, `graph_get_props`, `graph_set_props`, `grant_capability`, `irq_bind`, `dma_map`, `dma_submit`, `dma_wait`, `sys_log`, `dev_read`, `dev_write`, `spawn`
  - Category: FFI
  - Risk: medium
  - Description: These functions cast user pointers (u64) to Rust slices. Now use `validate_user_slice` helper to enforce range checks (user space only).

- `copy_out_slice`
  - Category: MEMORY
  - Risk: medium
  - Description: Copies data from kernel buffer to user buffer. Now uses `validate_user_slice_mut` for destination check.

- `init_syscall`
  - Category: ARCH
  - Risk: high
  - Description: Writes to MSRs (EFER, STAR, LSTAR, SFMASK) to configure `syscall` instruction.

### Memory Management (`kernel/src/mm/`)

- `bump_allocator.rs::BumpAllocator::init`
  - Category: MEMORY
  - Risk: medium
  - Description: Initializes the bump allocator with heap start and size.

- `bump_allocator.rs::BumpAllocator::alloc` / `dealloc`
  - Category: MEMORY
  - Risk: high
  - Description: Implementation of `GlobalAlloc`. Manages raw pointers for heap allocation.
  - Note: Added SAFETY comments explaining bounds checks.

- `debug_alloc.rs::DebugAlloc::alloc` / `dealloc`
  - Category: MEMORY
  - Risk: medium
  - Description: Wrapper around another allocator for debugging.

- `allocator.rs::init_paging`
  - Category: MEMORY
  - Risk: high
  - Description: Sets up the active level 4 page table.

- `allocator.rs::active_level_4_table`
  - Category: MEMORY
  - Risk: high
  - Description: Accesses the active level 4 page table using physical memory offset.
  - Note: Added SAFETY comments explaining preconditions.

- `pools.rs::PageTableAllocator`
  - Category: MEMORY
  - Risk: medium
  - Description: Implements `FrameAllocator`.

- `mirror_region.rs`
  - Category: MEMORY
  - Risk: medium
  - Description: Likely involves mapping physical memory regions.

### Bootloader (`kernel/src/bootloader.rs`)

- `bootloader.rs` (various)
  - Category: MEMORY
  - Risk: medium
  - Description: Parses boot information (memory map, framebuffer) provided by Limine. Uses `from_raw_parts`.
  - Note: Added SAFETY comments trusting bootloader.

### Main (`kernel/src/main.rs`)

- `kmain` (implied `no_mangle`)
  - Category: FFI
  - Risk: low
  - Description: Entry point called by bootloader.

- `drivers::serial::raw_write`
  - Category: OTHER
  - Risk: low
  - Description: Writes to serial port for logging.

### System (`kernel/src/system/`)

- `panic.rs`
  - Category: OTHER
  - Risk: low
  - Description: Panic handler, likely uses unsafe for halting or printing.
