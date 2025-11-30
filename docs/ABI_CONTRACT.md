# ABI Contract

This document clarifies the only supported contract between:

- **Kernel ↔ userland bundles** on bare metal.
- **Host tooling ↔ userland crates** when running with the `host` feature (`compositor/src/bin/host_compositor.rs`).

All higher-level components (drivers, compositor, apps, host browser shims) must treat the ABI defined in `thing_abi/src/lib.rs` as the single source of truth. Nothing else is stable.

## What the ABI guarantees today

- **Request/response compatibility**: `AbiRequest`/`AbiResponse` enums are shared by kernel and host runtimes. `userland/src/runtime.rs` serializes requests the same way in both environments.
- **Graph identity**: `GraphThing`, `GraphEdge`, `Map`, and `Value` have identical layouts on both sides. App code can rely on those structures without cfg-gating.
- **Framebuffer access**: `sys::fb_info`/`sys::fb_map` (bare metal) expose the Limine framebuffer as raw memory. Host mode never exposes raw video memory; it only implements the `FramebufferDevice` trait.
- **Device syscalls**: `dev_open`, `dev_read`, `dev_write`, and `dev_map` bridge kernel drivers to userland drivers. Host tooling does not implement these because it runs entirely in user space.
- **Watch semantics**: Watches deliver `GraphChange::Thing`/`GraphChange::Edge` batches. Every change carries the latest revision so apps can detect missed updates.
- **Task isolation**: Bare-metal tasks each own a per-task kernel stack, CR3, and bundle ID. The ABI expects user-mode pointers to be validated by the MMU; faults kill the task, not the kernel.

## What the ABI explicitly does *not* guarantee

- **Storage durability**: Neither backend persists the journal or graph yet. Host mode loses state on exit; bare-metal state vanishes on reboot.
- **Capability enforcement in host mode**: Only the kernel validates bundle ownership or capability graphs. Host mode responds with `CapabilityGranted { granted: true }` for development convenience.
- **Host input fidelity**: Browser input injected through `/input` never becomes graph Things today. Apps must not depend on host-specific behavior.
- **Framebuffer semantics**: The compositor only knows about a `FramebufferDevice` trait. Whether that device wraps Limine memory, an HTTP server, or a debugger stub is intentionally abstract.
- **Neo4j schema stability**: The optional `thing_host/neo4j` feature is experimental and does not implement kind filtering, schema migrations, or capability checks.
- **Snapshot availability**: `GraphSnapshot`/`GraphWatchBatch::changes_since` are internal kernels structures. There is no ABI to export/import them yet.

## Stable vs. experimental surface

| Category | Stability | Notes |
| --- | --- | --- |
| `AbiRequest::Fiat`, `Link`, `Get`, `Query`, `Watch*`, `Props*`, framebuffer syscalls | **Stable** for both environments. These requests are exercised by the compositor and drivers on every boot. |
| `FindByKind` | **Mostly stable**: kernel honors the `kind` filter; host runtime still returns the entire graph (to be fixed). |
| `GrantCapability` | **Stable on bare metal**, **stubbed on host**. Apps can issue the request but must not rely on host enforcement. |
| Device syscalls (`dev_*`, `irq_*`, `dma_*`) | **Bare-metal only**. Host mode never exposes raw devices. |
| Snapshot/delta APIs | **Experimental**. Structures exist but there is no ABI yet. |

## Design expectations for contributors

- Do **not** special-case kernel vs. host inside apps or the compositor. Everything must flow through `thing_abi::ThingRuntime`.
- Treat browsers, QEMU windows, and physical framebuffers as `FramebufferDevice`s. The compositor (`compositor/src/lib.rs`) should never inspect HTTP headers or SDL handles.
- When you need new data across the boundary, add a new ABI request or extend the existing structs. Reaching around the ABI (global statics, debug-only hooks) is considered a regression.
- If you change syscalls or ABI types, update both implementations (`kernel` and `thing_host`) and refresh this document.

Keeping this contract honest ensures the host runtime stays a reliable stand‑in for the kernel and prevents the compositor/apps from depending on implementation details that only exist in one environment.
