# AGENTS Instructions

## Formatting
- Use four spaces for indentation in all Rust files.
- Run `cargo fmt --all` inside each crate (`kernel/`, `compositor/`, `apps/*/`, and `userland/`) before committing changes.
- If `cargo fmt` complains that `rustfmt` is missing install it for your host toolchain, e.g. `rustup component add rustfmt` or `rustup component add --toolchain nightly-x86_64-unknown-linux-gnu rustfmt`.

## Programmatic checks
- Build the OS image by running `make` from the repository root.  This downloads the Limine bootloader and builds the kernel plus all userland ELF modules (drivers, compositor, and apps).
- Ensure the `xorriso` utility is installed (`apt-get install xorriso` on Debian based systems) otherwise the ISO creation will fail.
- Install the `x86_64-unknown-none` target with `rustup target add x86_64-unknown-none`.

## Building and running
- The provided `GNUmakefile` drives the build.  `make` builds the kernel and userland runner/compositor and produces an ISO image.
- Run `make run` to boot the ISO in QEMU.  The variable `KARCH` selects the architecture (default `x86_64`).
- Run `make clean` to remove build artifacts.

Important: When running `make run`, prefer to run it with a timeout so QEMU exits automatically and you don't have to manually close or kill the QEMU window. Example: `timeout 300 make run`.

## Repository layout
- `kernel/` – Rust kernel crate.  `kmain` in `src/main.rs` is the entry point.
- `compositor/` – userland compositor binary and library.
- `userland/` – shared userland support library (syscalls/graph helpers).
- `apps/` – small user apps (clouds, hello, clock) that publish window buffers.
- `limine.conf` describes boot configuration and modules included in the ISO.

## Device & Driver Philosophy (Critical)

The kernel only exposes **raw hardware streams and memory regions** through
generic device syscalls.

- Kernel code may:
  - Handle interrupts
  - Buffer raw bytes
  - Map framebuffer memory into user space

- Kernel code must NOT:
  - Decode scancodes
  - Interpret mouse packets
  - Draw text
  - Emit semantic graph events
  - Implement line disciplines or higher-level protocols

All such policy belongs strictly in **userland driver bundles**.

If semantic behavior appears in the kernel, it is a regression.

## Pull request guidelines
- Summarize user visible changes in the PR description.
- Mention whether `make` succeeded or failed in the testing section.
- Organize your commits meaningfully, with excellent but succinct commit messages.

## Architecture Notes & Pitfalls

### Syscall Stack Corruption (The "RIP=0x0" Bug)
A frequent regression in this codebase involves user-mode tasks crashing with `RIP=0x0` or random addresses after running for a short time.

**The Cause:**
The `syscall` instruction does not automatically switch stacks. If the kernel uses a single global stack (e.g., `static mut SYSCALL_KERNEL_STACK`) for the syscall entry point, a race condition occurs:
1. Task A enters a syscall and switches to the global stack.
2. Task A is preempted (e.g., timer interrupt) while on this stack.
3. Task B is scheduled, enters a syscall, and *also* uses the same global stack, overwriting Task A's saved state.
4. When Task A resumes, its stack is corrupted, leading to a crash (often returning to 0x0).

**The Fix:**
- **Per-Task Kernel Stacks:** Every task must have its own kernel stack.
- **Context Switch Updates:** The scheduler must update a pointer (e.g., `KERNEL_STACK_PTR` in `gdt.rs`) to the current task's kernel stack top whenever it switches tasks.
- **Assembly Trampoline:** The `syscall` entry assembly (`syscall_entry.S`) must read this pointer to find the correct stack to use.
- **State Preservation:** The user's stack pointer (`RSP`) must be saved onto this kernel stack immediately so it survives context switches.

**Pitfalls to Avoid:**
- **Global Mutable State:** Never use a single static buffer for execution stacks in a multitasking environment.
- **Interrupt Safety:** Remember that interrupts can fire at *any* instruction (unless `CLI` is used). If a resource (like a stack) is shared, it must be protected or unique per thread.
- **Syscall vs. Interrupt:** `syscall` is lighter than `int 0x80` but requires more manual state management. It does not save `RSP` or `RIP` on the stack automatically; it puts them in `RCX` and `R11`.

**CR3 Context Rule:**
Any kernel code that dereferences user pointers must assume that **CR3 is the active user page table at the time of execution**.
Kernel code must never assume that pointers are valid without explicit validation.
Invalid user pointers must fault the *task*, not the kernel.
