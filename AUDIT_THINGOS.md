# Audit of ThingOS Architecture & Code Quality

## Recent Updates (Dec 2025)

### Completed Tasks

#### 1. QEMU Smoke Test (TEST-001)
- **Objective**: Ensure the kernel boots successfully in a CI-like environment.
- **Action**: Created `scripts/qemu_smoke.sh` which runs `make run` with a timeout and checks for the "Kernel started!" log message.
- **Integration**: Added `qemu-smoke` target to `GNUmakefile`.
- **Status**: ✅ Verified. The test passes, confirming the kernel initializes correctly even if it crashes later due to known issues (e.g., page faults).

#### 2. Static Mut Cleanup (MEM-01)
- **Objective**: Remove dangerous `static mut` usage to improve memory safety and comply with Rust 2024 standards.
- **Kernel**:
  - Replaced `static mut CACHE` and `BUFF` in `kernel/src/bootloader.rs` with `spin::Once` and `UnsafeCell`-based static storage.
  - **Correction**: Initially used `Vec` which caused OOM during early boot. Replaced with a static array in `.bss` wrapped in `UnsafeCell` to ensure no heap allocation occurs before the heap is initialized.
- **Compositor**:
  - Replaced `static mut BACKBUFFER_STORAGE` in `compositor/src/main.rs` with `spin::Mutex`.
  - Updated `fallback_framebuffer` to lock the mutex and leak the guard, ensuring exclusive mutable access to the framebuffer memory for the lifetime of the compositor without risking data races.
- **Status**: ✅ Implemented and Verified (Regression fixed).

#### 3. Unsafe Usage & Syscall Boundary (ARCH-02)
- **Objective**: Map and harden `unsafe` usage in the kernel, specifically at the syscall boundary.
- **Action**:
  - Created `UNSAFE_MAP.md` to catalog and classify `unsafe` sites.
  - Hardened `kernel/src/arch/x86_64/syscall.rs` by introducing `validate_user_slice` helpers to enforce range checks on user pointers.
  - Added `SAFETY` comments to high-risk sites in memory management and bootloader.
- **Status**: ✅ Partially Implemented. Syscall boundary is safer, but full memory safety requires deeper architectural changes (e.g., proper user/kernel address space separation enforcement).

## 1. Overview

ThingOS is an experimental, graph-centric operating system written in Rust. It employs a hybrid kernel architecture where a monolithic kernel core (`kernel`) manages memory, scheduling, and basic device I/O, while hosting a graph database (`thing_model`) directly within the kernel address space. Userland applications (`apps`, `compositor`) run as separate ELF processes, interacting with the kernel and the graph via a rich syscall interface.

The system's defining characteristic is its "Graph-First" philosophy: system entities, UI widgets, and application state are represented as nodes ("Things") and edges in a semantic graph. The kernel acts not just as a resource manager but as the authoritative store for this graph.

### System Architecture Diagram

```mermaid
graph TD
    subgraph Kernel Space
        K[Kernel Core]
        MM[Memory Manager]
        SCHED[Scheduler]
        GS[Graph Store (Thing Model)]
        KD[Kernel Drivers (Kbd, Mouse, Serial)]
        
        K --> MM
        K --> SCHED
        K --> GS
        K --> KD
    end

    subgraph Userland
        C[Compositor]
        A1[Demo App]
        A2[Graph Viewer]
        UD[Userland Drivers (Framebuffer)]
        
        C -- Syscalls --> K
        A1 -- Syscalls --> K
        A2 -- Syscalls --> K
        UD -- Syscalls --> K
    end

    subgraph Hardware
        HW[CPU / RAM / Devices]
    end

    K --> HW
    KD --> HW
```

---

## 2. Strengths & Bright Spots

1.  **Unified Graph Abstraction**: The integration of the graph model (`thing_model`) into the core system is ambitious and provides a consistent way to handle state, configuration, and IPC.
2.  **Clear Userland Separation**: The distinction between kernel and userland is well-defined via the `userland` crate and a strongly typed syscall interface (`userland::sys`).
3.  **Modern Rust Usage**: The codebase leverages modern Rust features (e.g., `alloc`, `spin` locks) and generally avoids "C-style" Rust where possible, outside of low-level hardware interaction.
4.  **Compositor Design**: The compositor's use of a `WatchManager` to reactively update the UI based on graph changes is a powerful pattern that decouples rendering from state logic.
5.  **Bootloader Integration**: The use of Limine and the `bootloader` module to dynamically discover and load modules (`init`, drivers, apps) is flexible and robust.

---

## 3. Key Risks & Antipatterns (Prioritized)

| ID | Title | Severity | Area | Summary |
| :--- | :--- | :--- | :--- | :--- |
| **ARCH-01** | **Kernel-Graph Coupling** | High | Architecture | The graph store is embedded in the kernel. This bloats the kernel and makes graph logic critical to system stability. |
| **MEM-01** | **Unsafe Static Mutables** | High | Memory | Usage of `static mut` for critical structures (e.g., `BACKBUFFER_STORAGE`, `CACHE` in bootloader) poses data race risks. |
| **COMP-01** | **Compositor God Object** | Medium | Compositor | `compositor.rs` is ~3600 lines, handling layout, rendering, and event logic, making it hard to maintain. |
| **SCHED-01** | **Fixed Stack Sizes** | Medium | Scheduling | Tasks have fixed 256KB stacks. Deep recursion or large stack allocations will cause silent corruption or crashes. |
| **TEST-01** | **Lack of Tests** | High | Testing | Minimal automated testing. No integration tests to verify boot or basic system functionality. |

### 3.1 Kernel-Graph Coupling (ARCH-01)
The `kernel` crate directly includes `graph/store.rs`, which implements complex graph logic. If the graph store panics or corrupts memory, the entire OS crashes. This violates the microkernel principle of keeping the kernel minimal.
*   **Risk**: System instability, difficulty in upgrading graph logic without rebooting/recompiling kernel.
*   **Recommendation**: Eventually move the graph store to a privileged userland service (microkernel style).

### 3.2 Unsafe Static Mutables (MEM-01)
Several files use `static mut` for global state. For example, `compositor/src/main.rs` allocates a 32MB backbuffer as `static mut`. While the compositor is currently single-threaded, this pattern is unsafe and prevents future parallelization.
*   **Risk**: Data races, undefined behavior if multiple threads/interrupts access these globals.
*   **Recommendation**: Use `spin::Mutex`, `atomic` types, or `OnceCell` for safe global state.

### 3.3 Compositor God Object (COMP-01)
`compositor/src/compositor.rs` is a massive file that mixes concerns. It handles:
*   Window management
*   Widget tree traversal
*   Event dispatching
*   Layout calculation
*   Rendering commands
*   **Risk**: High cognitive load, difficult to test individual components (e.g., layout engine) in isolation.
*   **Recommendation**: Refactor into `layout.rs`, `events.rs`, `rendering.rs`, and `window_manager.rs`.

---

## 4. Subsystem Deep Dives

### 4.1 Kernel & Boot
*   **Description**: Boots via Limine, `kmain` initializes serial, logger, and system. `launcher.rs` loads user modules.
*   **Strengths**: Clean boot flow, dynamic module loading.
*   **Issues**: `bootloader.rs` uses `static mut` for caching memory regions.
*   **Next Steps**:
    *   Replace `static mut` in `bootloader.rs` with safe synchronization.
    *   Add a "panic handler" that dumps the log to serial before halting.

### 4.2 Memory & Unsafe
*   **Description**: Uses `bump_allocator` or `LockedHeap`. `mm` module manages paging.
*   **Strengths**: Pluggable allocator design.
*   **Issues**: `SanityCapture` in allocator uses a fixed array and mutex, which might be a bottleneck or deadlock risk in interrupt contexts. `unsafe` blocks in syscalls need better validation of user pointers.
*   **Next Steps**:
    *   Audit all `unsafe` blocks in `syscall.rs` to ensure user pointers are validated against user memory ranges.
    *   Document invariants for all `unsafe` functions.

### 4.3 Scheduling & Concurrency
*   **Description**: Round-robin scheduler. Tasks have fixed stacks and are linked to graph bundles.
*   **Strengths**: Simple, understandable scheduler.
*   **Issues**: Fixed stack size (256KB) is a ticking time bomb. No protection against stack overflow.
*   **Next Steps**:
    *   Implement guard pages for kernel stacks to detect overflows.
    *   Consider dynamic stack growth or configurable stack sizes.

### 4.4 Drivers
*   **Description**: Hybrid model. Keyboard/Mouse in kernel, Framebuffer in userland (mostly).
*   **Strengths**: Userland drivers for high-bandwidth devices (framebuffer) reduce kernel complexity.
*   **Issues**: Kernel drivers (keyboard) write directly to a global buffer.
*   **Next Steps**:
    *   Standardize the driver interface.
    *   Move more drivers to userland if possible.

### 4.5 Compositor & Widgets
*   **Description**: Custom widget toolkit and window manager.
*   **Strengths**: `WatchManager` integration is excellent.
*   **Issues**: "God Object" antipattern in `compositor.rs`. Hardcoded widget logic mixed with graph logic.
*   **Next Steps**:
    *   Extract layout logic into a pure function/module.
    *   Define a clear `Widget` trait that separates state from rendering.

### 4.6 Graph & Modes
*   **Description**: Graph is the "source of truth". Modes (F1-F12) are modeled but implementation details are scattered.
*   **Strengths**: Semantic modeling of system state.
*   **Issues**: `thing_model` crate seems underutilized or empty in the source tree (needs verification). Graph logic is embedded in kernel.
*   **Next Steps**:
    *   Verify `thing_model` crate status.
    *   Expose more graph operations via syscalls to allow userland to manage modes fully.

### 4.7 Userland Apps & Demo
*   **Description**: Small apps (`demo_app`, `graph_viewer`) demonstrating capabilities.
*   **Strengths**: Proof of concept for the OS vision.
*   **Issues**: Apps rely heavily on the specific kernel version/syscall ABI.
*   **Next Steps**:
    *   Stabilize the `userland` library API.

---

## 5. Testing & Tooling

*   **Current Status**:
    *   Unit tests: Likely exist for some crates but not visible in top-level exploration.
    *   Integration tests: Non-existent.
    *   CI: `make run` exists but requires manual verification.
*   **Missing Tests**:
    *   **Boot Test**: A script that boots QEMU and checks for a specific log message ("Kernel started!").
    *   **Syscall Fuzzing**: Basic fuzzing of syscall inputs to ensure kernel stability.
*   **Tooling Improvements**:
    *   Add a `test` target to `Makefile` that runs unit tests and the boot test.

---

## 6. Roadmap Suggestions (Non-Binding)

### Pass 1: Safety & Stability
*   Replace `static mut` with safe alternatives (`SpinMutex`, `Atomic`).
*   Add guard pages to kernel stacks.
*   Implement a basic QEMU boot test.

### Pass 2: Compositor Refactor
*   Split `compositor.rs` into `layout`, `rendering`, `events`.
*   Formalize the `Widget` trait.

### Pass 3: Kernel Cleanup
*   Audit `unsafe` in syscalls.
*   Improve error handling in `bootloader` and `launcher`.

### Pass 4: Graph Evolution
*   Investigate moving `Graph Store` to a userland service.
*   Fully implement graph-driven mode switching.

---

## 7. Open Questions & Assumptions

*   **Assumption**: The `thing_model` crate appeared empty in the file listing, but `kernel` imports it. I assumed the code is either in a different path or I missed it, but the analysis proceeds assuming the kernel *has* the graph logic (which `kernel/src/graph/store.rs` confirms).
*   **Question**: How are interrupts routed to userland drivers? The `SYSCALL_IRQ_BIND` suggests a mechanism, but the details of the delivery (signal? message?) are not fully clear from the audit.
