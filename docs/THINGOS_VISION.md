# ThingOS Vision and Roadmap

This document summarizes the intended architecture so agents can pick up work without context loss.

## Pillars

- **Everything is a Thing**: uniform data unit with stable ID, kind, fields, and externalized state (no in-place mutation). Declarative, inspectable, serializable.
- **The Graph is the system**: a directed, labeled multigraph; nodes are Things, edges capture containment, dependencies, supervision, IO streams, configuration, and subscriptions.
- **The Journal is the CPU**: append-only event log; components react to events and emit new ones. State is reconstructed by replay; persistence is the log.

## Current implementation (March 2025)

- `kernel/src/graph`
  - `canon`: human-readable symbols (`cc('K','B')`, etc.).
  - `journal`: append-only in-memory log with snapshot/replay hooks (capacity 1024 entries) and postcard export/import.
  - `graph`: Thing store scaffold (UUID-based) with a stub replay interpreter that turns events into symbol Things and edges.
- Drivers
  - Declarative descriptors in `drivers/registry.rs`; emit init/fail events to the journal.
  - Keyboard emits key press events (with scancode payloads); mouse emits move events (dx/dy/buttons payloads).
- System init (`system/inner.rs`) initializes telemetry after the heap and replays the journal stub into the graph.

## Gaps vs. vision

- Journal is in-memory only; export/import exists but no durable sink or on-boot replay of persisted events.
- Graph replay is a stub; edges and Things are not reconstructed from events.
- Things can be mutated in place; no versioned/immutable snapshots or revision IDs.
- No subscription/iterator API for consumers; no schema validation/dedup for kinds/predicates.
- Drivers/devices/configs are not yet represented as Things with edges.
- No snapshots/checkpointing for faster boot.

## Suggested next tasks

1) **Journal persistence and replay**
   - Define a minimal postcard/serde event format.
   - Add an append sink (memory/serial; later block device) and boot-time replay.
   - Add a simple subscription/iterator API.

2) **Graph reconstruction**
   - Interpret propositions into Thing creation/edges (predicates: contains/depends/supervises/streams/config-of).
   - Represent drivers/devices/configs/timers as Things with edges.

3) **Immutability/versioning**
   - Remove in-place `ThingData::Typed` mutation; treat updates as new versions (UUID + revision).
   - Mark dirty via events, not raw pointers.

4) **Schema and symbols**
   - Centralize kind/predicate registration with dedupe and docs.
   - Expand canon symbols for lifecycle, IO, error events.

5) **Snapshots**
   - Optional graph snapshots for faster boot; replay journal for convergence.

6) **Driver eventing**
   - Emit structured events for mouse moves, framebuffer init/flush, etc.
   - Consume events where applicable instead of direct globals.

Please keep edits aligned with these pillars: prefer declarative descriptors, event emission, and graph edges over bespoke mutable state.***
