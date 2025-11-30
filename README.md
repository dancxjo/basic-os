# ThingOS

An experimental Rust operating system plus a host-mode runtime for developing the userland stack without rebooting a VM. Today there are two ways to execute it:

- **Bare metal** via `make run`, which boots Limine, starts the kernel, launches the userland drivers/compositor/apps, and *currently* faults once the compositor touches the mapped framebuffer.
- **Host compositor** via `cargo run -p compositor --bin host_compositor --features host`, which runs the compositor logic on Linux/macOS, streams vector frames to a browser over HTTP/SVG, and treats browser events as the primary input path.

> **Current Architectural Direction (Nov 2025)**
>
> - The kernel is a *hardware and scheduling micro-core only*.
> - All input decoding, rendering, and I/O semantics live in userland bundles or host tooling.
> - The graph and journal live in the kernel on bare metal; the host runtime swaps in an in-memory graph store that mimics the ABI.
> - Every user task (bare metal) keeps an independent address space, CR3, and kernel stack that is mirrored into user CR3s.

## Documentation status

| Artifact | Status | Notes |
| --- | --- | --- |
| `README.md` | ✅ Accurate | Reflects the dual host/bare-metal reality and compositor/ABI model (this document). |
| `docs/GRAPH_API.md` | ✅ Accurate | Updated to enumerate the actually implemented syscalls and host vs. kernel behavior. |
| `docs/ABI_CONTRACT.md` | ✅ Accurate | New section spelling out the ABI guarantees and non-guarantees. |
| `docs/INPUT_AND_DEVICE_MODEL.md` | ✅ Accurate | Details on the unified device ABI and Native vs Hosted input models. |
| `docs/THINGOS_VISION.md` | ⚠️ Partially outdated | Still describes the aspirational state; keep as historical intent only. |

## Reality check: execution environments

ThingOS components only communicate through the ABI defined in `thing_abi/src/lib.rs`; both the kernel syscall veneer (`userland/src/runtime.rs`) and the host runtime (`thing_host/src/lib.rs`) implement the same request/response contract.

| Mode | Command | Graphics backend | Input source | Graph backend | Status |
| --- | --- | --- | --- | --- | --- |
| Bare metal | `timeout 300 make run` | `BitmapRenderer` + Limine framebuffer (`compositor/src/main.rs`, `compositor/src/framebuffer_backend.rs`) | PS/2/HID bytes exposed as raw devices, decoded by userland drivers (`drivers/keyboard_driver`, `drivers/mouse_driver`) | Kernel `Store` + journal (`kernel/src/graph/store.rs`, `kernel/src/graph/journal.rs`) | ❌ Broken: compositor trips a page fault/double fault once it blits the mapped framebuffer. |
| Host Linux | `cargo run -p compositor --features host` | `SvgRenderer` drawing into `HostFramebufferDevice` and served as `frame.svg` (`compositor/src/svg_backend.rs`) | HTTP injection to `/input/key` -> virtual FIFO -> userland driver | In-memory `GraphStore` inside `thing_host/src/store.rs` | ✅ Working development path. |
| Host + Neo4j | `GRAPH_BACKEND=neo4j ... cargo run ...` | Same SVG renderer/device | HTTP injection to `/input/key` | `Neo4jGraphStore` (`thing_host/src/store.rs`) with `neo4rs` pool selected via env + `GRAPH_BACKEND`. | ✅ Graph ABI backed by Neo4j; host/runtime/apps can’t tell which store is active. |

The browser never talks to the compositor directly; it is treated as a framebuffer device that fetches SVG snapshots and posts input events. The kernel has no awareness of HTTP, DOM events, or the fact that the framebuffer is virtual.

### Compositor architecture (vector-first, framebuffer-agnostic)

The compositor builds a device-neutral `Scene` (`compositor/src/lib.rs`) each tick:

- `Scene` stores a list of `SceneItem` commands such as `FillRect`, `DrawText`, `BlitImage`, or `DrawCursor`.
- A `RendererBackend` turns those vector commands into an artifact (`&[u32]` for bitmaps, `String` for SVG). The default implementations are `BitmapRenderer` (bare metal) and `SvgRenderer` (host). Adding another backend only requires implementing the trait.
- A `FramebufferDevice` receives renderer output and is responsible for presenting it. `BitmapFramebufferDevice` writes into the Limine-provided linear framebuffer; `HostFramebufferDevice` simply stores the latest SVG payload for the HTTP server.

Because the compositor only manipulates `Scene` commands it does **not** know whether the output will land on hardware, a QEMU window, or a browser `<svg>` element. Similarly, it only manipulates cursor state and window surfaces pulled from the graph—networking, HTTP servers, and browser plumbing are all handled in the host wrapper.

### Accessibility-first interfaces

Before we layer in features like scrolling, agents working on the compositor, drivers, or apps should explicitly check new interface surface changes against WCAG-inspired considerations so accessibility is never an afterthought. That means keeping things perceivable (text alternatives for icons, contrast checks, zoom-aware layouts), operable (keyboard navigation, focus traps, clear input affordances), understandable (consistent semantics, predictable state transitions, meaningful labels for status windows), and robust (exposing intent to assistive tech via the graph, not hiding behavior behind platform quirks).

Whenever a task touches the windowing scene graph, mouse/keyboard interactions, or framebuffer output, add a quick note about how the change respects those principles—especially focus management, scroll affordances, and how events get marshaled so screen readers or remote clients could interpret them in the future. Baking these checkpoints into the documentation keeps us honest while we build scrolling and other interface features from the ground up.

### Host vs. kernel responsibilities

| Area | Kernel (bare metal) | Host runtime / tooling | Shared via ABI |
| --- | --- | --- | --- |
| Graph storage | `kernel/src/graph/store.rs` keeps Things/edges plus the journal | `thing_host/src/store.rs` supplies an in-memory `GraphStore` (optional Neo4j feature is still experimental) | Requests/responses defined in `thing_abi/src/lib.rs` cover Fiat/Link/Get/Query/Find/Props/Watch/Capability. |
| Journaling | `kernel/src/graph/journal.rs` buffers propositions; not exposed to userland | No journal, all state is transient | Nothing: only the derived graph crosses the ABI boundary. |
| Framebuffer | Kernel exposes raw Limine framebuffer via `sys::fb_info`/`sys::fb_map` (`userland/src/sys.rs`) and keeps it mapped inside user CR3s. | Host treats the browser as the framebuffer device via HTTP/SVG and never informs the kernel. | Compositor publishes frames as Things (`canon::DISPLAY_FRAME`) regardless of backend; apps consume graph events only. |
| Input | Kernel exposes PS/2 bytes and IRQs via generic device syscalls; userland drivers decode bytes into graph events (`drivers/keyboard_driver`, `drivers/mouse_driver`). | Browser events are POSTed to `/input`; host compositor updates cursor state locally and does **not** emit graph Things yet. | Apps still only see graph Things, so the ABI boundary remains intact. |
| Graph capabilities | Kernel enforces bundle ownership and capability edges before granting (`kernel/src/graph/store.rs:262`). | Host runtime blindly returns `CapabilityGranted { granted: true }`. | ABI format is the same even though policy differs. |

### Graph ABI reality

See `docs/GRAPH_API.md` for the full surface. Highlights:

- **Implemented requests**: `Fiat`, `Link`, `Get`, `Query`, `FindByKind`, `WatchRegister`, `WatchPoll`, `PropsGet`, `PropsSet`, `GrantCapability`.
- **Partially implemented**:
  - `FindByKind` respects the stringified `Symbol` on bare metal (`kernel/src/graph/store.rs::find_by_kind`) but the host in-memory store currently returns *all* Things regardless of kind.
  - `WatchRegister` and `WatchPoll` work, yet `WatchUnregister` has no syscall path on bare metal—watches live until the task exits.
  - Host capability granting is an optimistic stub.
- **Declared but not wired**: snapshot export/import structures exist but there are no syscalls yet; `userland::graph::fiat_thing` is a placeholder and always returns `Uuid::nil()`.

Both compositor code paths use `userland/src/runtime.rs` to talk through the ABI—on bare metal it wraps raw syscalls, and on the host it forwards to `HostRuntime`.

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

### Running the host compositor

The host compositor reuses the exact userland stack with the `host` feature:

```bash
cargo run -p compositor --bin host_compositor --features host
```

This leaks a `thing_host::HostRuntime` into `userland`, resizes the virtual framebuffer automatically when the browser window resizes, and serves the SVG at `http://127.0.0.1:8080/frame.svg`. The browser presses the `/input` endpoint with JSON mouse/key events; today those only update the compositor’s cursor state.

### Neo4j-backed workflow

The host runtime can swap its graph store for a Neo4j instance without changing the ABI that the compositor, apps, and drivers see. A typical setup:

```bash
# 1. Start Neo4j locally (default creds neo4j/secret)
docker compose -f docker-compose.neo4j.yml up -d

# 2. Launch the host compositor with the neo4j feature enabled
GRAPH_BACKEND=neo4j \
NEO4J_URI=bolt://127.0.0.1:7687 \
NEO4J_USER=neo4j \
NEO4J_PASSWORD=secret \
cargo run -p compositor --bin host_compositor --features "host thing_host/neo4j"

# 3. In another terminal, exercise the ABI like a host app would
GRAPH_BACKEND=neo4j \
NEO4J_URI=bolt://127.0.0.1:7687 \
NEO4J_USER=neo4j \
NEO4J_PASSWORD=secret \
cargo run -p graph_client --features thing_host/neo4j --target x86_64-unknown-linux-gnu
```

`graph_client` uses the same userland graph helpers as real apps: it creates Things, links them, and performs both `find_by_kind` and pattern queries. You can confirm the nodes/edges via the Neo4j Browser (`http://127.0.0.1:7474`) while the host compositor keeps rendering through the host ABI.

Prefer one command? `make host-clouds-neo4j` runs the same flow end-to-end (Docker Neo4j, host compositor, and the Clouds demo app) against the Linux host using the Neo4j backend.

## Current limitations

- Bare-metal graphics still hit a page fault/double fault when the compositor blits into the Limine framebuffer; bring-up/debugging must happen in host mode for now.
- The host path is the primary way to exercise the compositor, graph, and apps. Browser input does **not** flow back into the graph yet, so demo apps can observe windows but cannot receive host keystrokes/mouse clicks.
- Neo4j support requires the `thing_host/neo4j` feature plus `GRAPH_BACKEND=neo4j`; it persists Things/edges through Neo4j but still stores props as JSON blobs and does not enforce capabilities yet.
- Host mode uses browser events as the sole input pipeline. Bare-metal input comes from PS/2/HID devices through userland drivers once the kernel stack bug is resolved.
- Graph snapshots/journaling are kernel-only; the host runtime keeps everything in memory and loses state on exit.

## License

MIT
