# ThingOS

An experimental Rust operating system. It currently boots with the Limine bootloader, sets up basic x86_64 hardware, and runs a toy user program. Persistence and other advanced services are still to come.

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

- **kernel/** – the Rust kernel crate. `system.rs` performs initialization:
  - sets up the GDT, paging, kernel stack and heap
  - enables the syscall mechanism and interrupt handling
  - initializes the framebuffer and PS/2 devices
  - creates an HPET/RTC based `Clock`
  - loads userland ELF modules (drivers, compositor, and a demo app)

  After these steps the kernel enables interrupts and starts a small
  cooperative scheduler. Modules listed in `limine.conf` are pulled in
  as separate tasks (drivers first, then compositor, then an app).
  Press `Scroll Lock` or rely on timer ticks to yield execution.
  Function keys `F1`–`F12` select which task runs next.

- **compositor/** – userland compositor binary/library. It ingests app
  `window_buffer_updated` events and produces composed frames. Shipped
  as its own ELF module instead of a combined `userland.bin`.
- **drivers/** – user-space device drivers (keyboard, mouse, framebuffer),
  each built as its own ELF module that the kernel loads directly.
- **apps/** – small demo apps (clouds, hello, clock). Each builds as a
  standalone ELF module that the kernel loads directly.
- **userland/** – shared userland support library (syscalls/graph helpers).

## Syscall surface (early)

Userland interacts with the system through graph and device syscalls. The
The journal stays inside the kernel; user code queries live state via
graph operations and watches instead of snapshots.

- `graph_find_by_kind(kind_ptr, kind_len, cursor)`: query the graph for things
  of a specific kind. Returns a paginated list of things.

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
- Drivers register declaratively and emit init/fail events; keyboard emits key press events into the journal. Drivers are Things too and should eventually appear as nodes with edges like `implements HardwareThing`, `streams IRQThing`, `depends_on ClockThing`, `supervises TaskThing`.
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
