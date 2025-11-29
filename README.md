# ThingOS

An experimental Rust operating system. It currently boots with the Limine bootloader, sets up basic x86_64 hardware, and runs a toy user program. Persistence and other advanced services are still to come.

> **Current Architectural Direction (Nov 2025)**
>
> - The kernel is a *hardware and scheduling micro-core only*.
> - All input decoding, rendering, and I/O semantics live in userland.
> - The graph and journal remain kernel-resident data systems, but never mediate hardware directly.
> - Every user task runs in an independent address space with its own CR3 and kernel stack.

## Building and running

The top level `GNUmakefile` builds an ISO image and runs it under QEMU.

```bash
# Build kernel and example program
make

# Boot the image in QEMU
make run
```

Important: When running `make run`, prefer to run it with a timeout so QEMU exits automatically and you don't have to manually close or kill the QEMU window. Example:

```
timeout 300 make run
```

`KARCH` can be set to `x86_64` (default) or other architectures supported by the Makefile such as `aarch64` and `riscv64`.

## Debugging with GDB

Run `make run-debug` to launch QEMU paused with a GDB stub on TCP port 1234. In
VS Code you can create a `launch.json` entry that attaches to this stub. Use a
`gdb-multiarch` or `gdb` executable and specify the kernel debug binary, e.g.
`kernel/target/x86_64-unknown-none/debug/kernel`, as the program. The debugger
should connect to `localhost:1234`.

Example terminal invocation:

```bash
make run-debug &
gdb-multiarch kernel/target/x86_64-unknown-none/debug/kernel -ex "target remote :1234"
```

## Code overview

**kernel/** – the Rust kernel crate. `system.rs` performs initialization:
  - sets up the GDT, paging, per-task kernel stacks, and heap
  - enables the syscall mechanism and interrupt handling
  - initializes low-level hardware shims (PS/2, framebuffer, serial) as raw devices
  - creates the system clock
  - loads userland ELF modules as independent tasks (drivers first, then compositor, then apps)

  After initialization the kernel enables interrupts and enters a preemptive round-robin scheduler.
  Each ELF module runs in its own address space with an independent CR3 and kernel stack.
  Timer interrupts drive task preemption; no task may assume exclusive CPU ownership.

- **compositor/** – userland compositor binary/library. It ingests app
  `window_buffer_updated` events and produces composed frames. Shipped
  as its own ELF module instead of a combined `userland.bin`.
- **drivers/** – *userland* device interpreters (keyboard, mouse, framebuffer).
  These consume **raw kernel devices** via generic device syscalls and translate byte streams
  and memory mappings into graph events and high-level behavior. Each is built as an independent ELF module.
- **apps/** – small demo apps (clouds, hello, clock). Each builds as a
  standalone ELF module that the kernel loads directly.
- **userland/** – shared userland support library (syscalls/graph helpers).

## Syscall Surface (Current)

Userland interacts with the kernel exclusively through:

- **Generic device syscalls** (`dev_open`, `dev_read`, `dev_write`, `dev_map`)
- **Task and scheduling syscalls**
- **Graph operations and watches** (purely as *data services*, not hardware mediation)

The kernel only exposes **raw byte streams and memory regions** for hardware devices.
All decoding, interpretation, buffering, and policy live entirely in userland drivers.

The journal remains an internal kernel implementation detail.
Userland never appends to or reads the journal directly.

## Current status

- kernel boots, sets up devices, and exposes graph/device syscalls
- compositor plus demo drivers/apps are launched as independent modules
- "Everything is a Thing" graph is wired via live graph operations and watches
- userland `WatchManager` drives app events

This repository is in a very early stage. Persistence and higher level
services are not implemented yet. A simple cooperative multitasking
system exists but remains experimental. Development is focused on
bringing up the core kernel.

## ThingOS vision (short)

- **Everything is a Thing**: uniform data unit with identity, kind, and fields. Should be declarative, inspectable, and serializable; state comes from events, not in-place mutation. Identities are stable and revisions accumulate (Things never “die”; they gain new versions).
- **The Graph is the system**: directed, labeled multigraph describing containment, dependencies, supervision, IO, config, and message streams between Things. The graph is not stored — it is derived by replaying the journal.
- **The Journal is the CPU**: append-only event log; components react to events and emit new ones. State is reconstructed by replay inside the kernel; userland observes the resulting graph via queries and watches.

Current implementation status:

- Graph core (kernel/src/graph): symbols (`canon`), append-only in-memory journal with snapshot/replay, and a Thing store/graph scaffold.
- Drivers register as Things and may emit init/fail events.
- **Kernel device shims never emit semantic events.**
- Userland drivers (e.g., keyboard driver) decode raw device streams and emit all semantic events (keypresses, mouse movement, etc.) into the graph and journal. Drivers are Things too and should eventually appear as nodes with edges like `implements HardwareThing`, `streams IRQThing`, `depends_on ClockThing`, `supervises TaskThing`.
- Replay hook is wired but does not yet rebuild the graph from the journal; journal is in-memory only.

Example event (journal proposition):

```
subject: keyboard0   predicate: pressed   object: key='a', scancode=0x1e
// canon form: (KB_PRESSED t:keyboard0 k:'a' sc:0x1e)
```

## Near-term roadmap

- Journal: introduce a durable sink (memory/serial/block when available), postcard/serde event format, and a replay pass on boot.
- Graph: interpret journal propositions into Thing/edge updates; formalize predicates (contains/depends/supervises/streams/config-of) and make drivers/devices/configs Things with edges.
- Immutability/versioning: remove in-place mutations of Thing data; treat updates as new versions/events with UUID+revision.
- Subscriptions: add a simple iterator/subscription API so components consume relevant journal events.
- Schema: centralize kind/predicate registration with dedupe and docs; expand symbol table for common lifecycle/IO/error events.
- Snapshots: optional graph snapshots for faster boot, with journal replay for convergence.

Contributors/agents: please keep these pillars in mind when adding drivers or services. Emit events instead of mutating globals; register Things and edges where possible; prefer declarative descriptors over bespoke wiring.

## Recent Fixes & Findings (Nov 2025)

- **Userland Logging**: Implemented `SYSCALL_LOG` (0x99) to enable `println!` in userland. This bridges userland logs to the kernel serial output, essential for debugging.
- **Framebuffer Info Fix**: Fixed a bug in `userland::sys::fb_info` where the syscall return value (struct size) was incorrectly checked against 0, causing `framebuffer_driver` to panic.
- **Compositor Crash Resolved**: Fixed a User Mode Page Fault in `compositor`. The crash was due to a combination of silent allocation failures and the `fb_info` bug.
- **Allocation Error Handling**: Added `#[alloc_error_handler]` to userland to ensure Out-Of-Memory (OOM) conditions cause an explicit panic instead of silent failure.

## License

MIT
