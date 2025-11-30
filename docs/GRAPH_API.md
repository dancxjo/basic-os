# Graph API

ThingOS treats the graph of Things (nodes) and Edges (relationships) as the central OS object model. On bare metal every mutation flows through the kernel journal (`kernel/src/graph/journal.rs`) and lands in the in-memory store (`kernel/src/graph/store.rs`). Userland never sees the journal; it only talks to the derived graph surface through the ABI defined in `thing_abi/src/lib.rs`. Host-mode development swaps the journal/store for `thing_host::GraphStore`, but the ABI stays identical.

## Layering recap

1. **Journal** (`kernel/src/graph/journal.rs`) is append-only and kernel-private.
2. **Graph store** keeps historical versions plus capability policy. Bare metal uses `kernel/src/graph/store.rs`; host mode uses the in-memory `thing_host/src/store.rs` (with an experimental Neo4j backend behind `thing_host/neo4j`).
3. **Userland API** (`userland/src/graph.rs`) is a façade that serializes requests, invokes either syscalls (bare metal) or the host runtime, and deserializes responses.

## ABI requests and their real status

| ABI request | Kernel behavior | Host runtime behavior | Notes |
| --- | --- | --- | --- |
| `Fiat` | Creates or updates Things, bumps revisions, emits journal propositions, enforces bundle ownership. | Inserts/updates in-memory Things. | Labels default to the supplied `kind` on both sides. |
| `Link`/`GraphThatRequest` | Adds edges with capability checks and journal events. | Adds edges without capability checks. | Host runtime assumes a trusted dev session. |
| `Get`/`Query` | Returns Things by UUID or `NodePattern`. | Same as kernel. | Patterns match labels (`Symbol`s) and literal props. |
| `FindByKind` | Filters by the stringified `Symbol` and pages results (`kernel/src/graph/store.rs::find_by_kind`). | Host runtime now filters by the same `Symbol` strings and returns cursors via the shared API. | Both implementations share the same wire format. |
| `WatchRegister`/`WatchPoll` | Registers watches by `NodePattern`, queues `GraphChange`s, drains via `GraphWatchBatch`. Unregister is not wired yet. | Same API plus working unregister; queues live in `HostRuntime`. | Watch payloads contain full Things/Edges, no diffs. |
| `PropsGet`/`PropsSet` | Reads or merges arbitrary maps and bumps revisions. | Reads/merges maps, no capability gating. | `PropsSet` is the only mutation path used by `graph_smoke`. |
| `GrantCapability` | Enforces bundle ownership and hardware capability restrictions (`kernel/src/graph/store.rs:262`). | Always returns `CapabilityGranted { granted: true }`. | Capability graphs are currently ignored on the host. |
| Snapshots / watch diffs | Structs exist, but no syscalls expose them. | Not implemented. | `GraphSnapshot` only lives inside the kernel today. |

## Things and edges

A Thing is a versioned entity stored as `thing_abi::GraphThing`:

```text
id (Uuid), kind (Symbol), labels (BTreeSet<Symbol>), fields (Map<Symbol, Value>), owner (BundleId), revision (u64)
```

Edges (`GraphEdge`) connect Things via `src`, `pred`, and `dst`, and also carry arbitrary `props`. The store keeps historical revisions of both Things and edges, but only the latest revision crosses the ABI today.

Common symbols live in `userland::canon`. For example, windows use `canon::WINDOW`, surfaces emit `canon::SURFACE`, and framebuffer metadata uses `canon::DISPLAY_FRAMEBUFFER`.

## Reading the graph

Use `userland::find_by_kind` and `userland::graph::get_nodes` to query:

```rust
use userland::{find_by_kind, graph::{NodePattern, GraphThing}};

let windows: Vec<GraphThing> = find_by_kind("window");
let mut pattern = NodePattern::default();
pattern.labels.push(userland::canon::SURFACE);
let surfaces = userland::graph::get_nodes(pattern);
```

Bare-metal queries are serviced by the syscall veneer in `userland/src/runtime.rs`; host-mode queries are forwarded to `HostRuntime`.

## Watching the graph

`WatchManager` (`userland/src/watch.rs`) registers `NodePattern`s and polls each tick:

```rust
use userland::{canon, watch::{ThingFilter, WatchManager}};

let mut watches = WatchManager::new();
let app_id = watches.register_app();
let window_watch = watches.register_graph(app_id, ThingFilter {
    kind: Some(canon::WINDOW),
    id: None,
});

watches.process_graph(&[app_id]);
for event in watches.drain_inbox(app_id) {
    println!("received {:?}", event);
}
```

On bare metal `WatchRegister`/`WatchPoll` map to `SYSCALL_GRAPH_WATCH_REGISTER`/`SYSCALL_GRAPH_WATCH_POLL`. Watch IDs currently remain active until the task exits; there is no syscall to unregister yet. The host runtime supports unregistering via `AbiRequest::WatchUnregister` but the userland helper does not expose it.

## Writing the graph

`userland::graph::fiat` creates Things; `userland::graph::that` adds edges:

```rust
use userland::{canon, graph};
use uuid::Uuid;

let mut fields = graph::map();
fields.insert(canon::TITLE, graph::Value::text("Hello"));
let window_id = graph::fiat(None, canon::WINDOW, fields);

let compositor = Uuid::new_v5(&Uuid::NAMESPACE_OID, b"compositor");
graph::that(window_id, canon::COMPOSED_BY, compositor, 0);
```

The higher-level app façade (`userland/src/app.rs`) uses the same primitives when creating windows, shared buffers, and surfaces.

### `Thingable`

Implement `Thingable` to decode strongly typed structs:

```rust
use userland::{canon, graph::{Thingable, GraphThing}};

#[derive(Clone)]
pub struct Window { pub title: String, pub width: u64 }

impl Thingable for Window {
    fn kind() -> &'static str { "window" }
    fn load(thing: &GraphThing) -> Option<Self> {
        if thing.kind != canon::WINDOW { return None; }
        Some(Window {
            title: thing.fields.get(&canon::TITLE)?.as_text()?.into(),
            width: thing.fields.get(&canon::WIDTH)?.as_u64()?,
        })
    }
}
```

`load_things_of_kind::<Window>()` will call `find_by_kind("window")` and decode each entry. Note that `userland::graph::fiat_thing` is still a placeholder (it always returns `Uuid::nil()`), so typed creation helpers are not ready yet.

## Syscalls used by the graph layer

All numeric IDs live in `userland/src/sys.rs`:

| Syscall | Purpose | Observations |
| --- | --- | --- |
| `SYSCALL_GRAPH_FIAT` (0x01) | Create/update Things. | Returns the revision ID as `u64`. |
| `SYSCALL_GRAPH_LINK` (0x02) | Create edges. | Userland handles retries if needed. |
| `SYSCALL_GRAPH_GET` (0x05) | Fetch Things by UUID/pattern. | Encodes `GraphGetRequest` with postcard. |
| `SYSCALL_GRAPH_WATCH_REGISTER` (0x06) | Register watches. | No unregister syscall yet. |
| `SYSCALL_GRAPH_WATCH_POLL` (0x07) | Drain watch queues. | Returns postcard-encoded `GraphWatchBatch`. |
| `SYSCALL_GRAPH_FIND_BY_KIND` (0x0B) | Stream Things by kind. | Host runtime currently ignores the kind filter. |
| `SYSCALL_GRAPH_GET_PROPS` (0x0E)`/`0x0F` | Get/set property maps. | Backed by `GraphPropsGetRequest` / `GraphPropsRequest`. |
| `SYSCALL_GRANT_CAPABILITY` (0x10) | Request capability edges. | Kernel enforces policy; host runtime currently stubs this out. |

## Limitations to be aware of

- Watch payloads only include full Things/Edges, not diffs. Applications must reconcile on their own.
- `find_by_kind` requires the canonical symbol string (e.g., `"window"`), which works on bare metal but is ignored in host mode for now.
- Host mode lacks journaling entirely, so restarting the host compositor clears the graph.
- Neither backend exposes snapshots or delta streams yet despite the presence of `GraphSnapshot` and `GraphWatchBatch::changes_since` types.

For the contract shared between the kernel, host runtime, compositor, and apps refer to `docs/ABI_CONTRACT.md`.
