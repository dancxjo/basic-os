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

`KARCH` can be set to `x86_64` (default) or other architectures supported by the Makefile such as `aarch64` and `riscv64`.

## Debugging with GDB

Run `make run-debug` to launch QEMU paused with a GDB stub on TCP port 1234. In
VS Code you can create a `launch.json` entry that attaches to this stub. Use a
`gdb-multiarch` or `gdb` executable and specify the kernel debug binary, e.g.
`kernel/target/x86_64-unknown-none/debug/thingos`, as the program. The debugger
should connect to `localhost:1234`.

Example terminal invocation:

```bash
make run-debug &
gdb-multiarch kernel/target/x86_64-unknown-none/debug/thingos -ex "target remote :1234"
```

## Code overview

- **kernel/** – the Rust kernel crate. `system.rs` performs initialization:
  - sets up the GDT, paging, kernel stack and heap
  - enables the syscall mechanism and interrupt handling
  - initializes the framebuffer and PS/2 devices
  - creates an HPET/RTC based `Clock`
  - loads the `compositor` ELF binary as a user task

  After these steps the kernel enables interrupts and starts a small
  cooperative scheduler. At boot the scheduler jumps into the userland
  compositor (`compositor`), which hosts the compositor library and demo apps. Press `Scroll Lock` or rely on timer ticks
  to yield execution. Function keys `F1`–`F12` select which task runs next.

- **compositor/** – userland compositor library. It ingests app
  `window_buffer_updated` events and produces composed frames.
- **apps/** – small demo apps (clouds, hello, clock) that publish window
  buffers/events to be composed.
- **userland/** – shared userland support library (syscalls/telemetry helpers).
- **runner/** – userland binary that wires the compositor and demo apps
  together for now (until multiple user tasks are supported).

## Syscall surface (early)

- `journal_emit(kind, ptr, len)`: append an event to the telemetry journal. The
  payload is parsed as postcard-serialized `Value` when possible, otherwise as
  UTF-8 text or raw bytes. `write` events (symbol `WRT`) are also reflected to
  the console for convenience.
- `journal_snapshot(out_ptr, out_len)`: copy the postcard-serialized journal
  into a user buffer. The return value is the required size; if the provided
  buffer is too small no data is written.
- `graph_find_by_kind(kind_ptr, kind_len, cursor)`: query the graph for things
  of a specific kind. Returns a paginated list of things.

## Current status

- kernel boots, sets up devices, and exposes journal/graph/dev syscalls
- compositor launches four sample apps
- "Everything is a Thing" graph is wired via journal events and snapshots
- userland `WatchManager` drives app events

This repository is in a very early stage. Persistence and higher level
services are not implemented yet. A simple cooperative multitasking
system exists but remains experimental. Development is focused on
bringing up the core kernel.

## ThingOS vision (short)

- **Everything is a Thing**: uniform data unit with identity, kind, and fields. Should be declarative, inspectable, and serializable; state comes from events, not in-place mutation. Identities are stable and revisions accumulate (Things never “die”; they gain new versions).
- **The Graph is the system**: directed, labeled multigraph describing containment, dependencies, supervision, IO, config, and message streams between Things. The graph is not stored — it is derived by replaying the journal, with optional snapshots for faster boot.
- **The Journal is the CPU**: append-only event log; components react to events and emit new ones. State is reconstructed by replay; persistence is the log.

Current implementation status:

- Telemetry core (kernel/src/telemetry): symbols (`canon`), append-only in-memory journal with snapshot/replay, and a Thing store/graph scaffold.
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
## License

MIT
