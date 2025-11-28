# Telemetry Graph API

ThingOS uses a versioned graph of "Things" (nodes) and Edges (relationships) as the core data model. All state changes flow through an append-only journal, and the kernel maintains a derived graph view that userland applications can query.

## Overview

The telemetry stack consists of three layers:

1. **Journal** (`kernel/src/graph/journal.rs`, `userland/src/lib.rs`): Append-only event log
2. **Graph** (`kernel/src/graph/store.rs`): Derived view built by replaying journal events
3. **Userland API** (`userland/src/lib.rs`): High-level typed facade for working with Things

## Core Concepts

### Things

A Thing is a versioned entity in the graph with:
- `id`: Unique UUID identifier
- `kind`: Symbol describing the type (e.g., `WINDOW`, `PROCESS`, `DRIVER`)
- `fields`: Key-value map of properties (using `Symbol` keys and `Value` values)
- `revision`: Version number for tracking updates

### Edges

An Edge represents a directed relationship between two Things:
- `src`: Source Thing UUID
- `pred`: Predicate Symbol describing the relationship (e.g., `COMPOSED_BY`, `OWNS`)
- `dst`: Destination Thing UUID
- `revision`: Version number

### Graph Queries

The graph can be queried for specific Things by kind, or by ID.

## Userland API

### Reading the Graph

```rust
use userland::{find_by_kind, load_thing};

// Find all things of a specific kind
let windows = find_by_kind("window");
println!("Found {} windows", windows.len());

for thing in windows {
    println!("Window {} with fields {:?}", thing.id, thing.fields);
}

// Load a specific thing by ID
if let Some(thing) = load_thing::<Window>(some_id) {
    println!("Loaded window: {:?}", thing);
}
```

### Writing to the Graph

Use `fiat` to create or update Things, and `that` to create Edges:

```rust
use userland::{canon, fiat, that, map, Value};
use uuid::Uuid;

// Create a Thing with explicit ID
let id = Uuid::new_v5(&Uuid::NAMESPACE_OID, b"my-window");
let mut fields = map();
fields.insert(canon::NAME, Value::text("My Window"));
fields.insert(canon::STATUS, Value::symbol(canon::INIT));
fiat(Some(id), canon::WINDOW, fields);

// Create an Edge
let compositor = Uuid::new_v5(&Uuid::NAMESPACE_OID, b"compositor");
that(id, canon::COMPOSED_BY, compositor, 0);
```

### The Thingable Trait

For type-safe, ergonomic access to Things, implement the `Thingable` trait:

```rust
use userland::{Thingable, Symbol, Value, canon};
use alloc::collections::BTreeMap;

#[derive(Debug, Clone, PartialEq)]
pub struct Window {
    pub title: String,
    pub x: u64,
    pub y: u64,
    pub width: u64,
    pub height: u64,
}

impl Thingable for Window {
    fn kind() -> &'static str {
        "window"
    }

    fn load(thing: &GraphThing) -> Option<Self> {
        if thing.kind != canon::WINDOW {
            return None;
        }
        Some(Window {
            title: thing.fields.get(&canon::TITLE)?.as_text()?.to_string(),
            x: thing.fields.get(&canon::X)?.as_u64()?,
            y: thing.fields.get(&canon::Y)?.as_u64()?,
            width: thing.fields.get(&canon::WIDTH)?.as_u64()?,
            height: thing.fields.get(&canon::HEIGHT)?.as_u64()?,
        })
    }
}
```

### Thingable Helpers

Once you've implemented `Thingable`, use these helpers:

```rust
use userland::{fiat_thing, load_thing, load_things_of_kind, update_thing};

// Create a Thing from a typed value (ID is deterministic)
let window = Window {
    title: "Hello".to_string(),
    x: 100, y: 200,
    width: 800, height: 600,
};
let id = fiat_thing(&window);

// Load a Thing by ID and decode it
if let Some(window) = load_thing::<Window>(id) {
    println!("Loaded window: {}", window.title);
}

// Load all Things of a given kind
let windows = load_things_of_kind::<Window>();
for (id, window) in windows {
    println!("Window {}: {}", id, window.title);
}

// Update a Thing (automatically increments revision)
let mut updated = window.clone();
updated.width = 1024;
updated.height = 768;
update_thing(id, &updated);
```

## Design Principles

1. **Journal is the source of truth**: All state changes are expressed as events in the append-only journal.
2. **Graph is derived**: The kernel graph is built by replaying journal events; it can be reconstructed at any time.
3. **No kernel-side pointers**: Only `Value` trees and UUIDs cross the kernel-userland boundary.
4. **Userland caching is optional**: Applications can query via `find_by_kind()` or maintain their own process-local caches.
5. **Revisions enable conflict detection**: Each Thing and Edge has a revision number for tracking updates.

## Example Application

See `apps/graph_demo/` for a complete example that demonstrates:
- Creating a Window using `fiat_thing`
- Querying the graph with `graph_snapshot`
- Loading all Windows with `load_things_of_kind`
- Updating a Thing with `update_thing`

## Syscalls

The following syscalls are exposed to userland:

### Graph Operations
- `SYSCALL_GRAPH_FIAT` (0x01): Create a new Thing in the graph
- `SYSCALL_GRAPH_LINK` (0x02): Create an edge between two Things
- `SYSCALL_GRAPH_GET` (0x05): Get a specific Thing by UUID
- `SYSCALL_GRAPH_FIND_BY_KIND` (0x0B): Find Things by kind
- `SYSCALL_GRAPH_GET_NODES` (0x0D): Get nodes matching a pattern
- `SYSCALL_GRAPH_GET_PROPS` (0x0E): Get properties of a node
- `SYSCALL_GRAPH_SET_PROPS` (0x0F): Set properties on a node

### Watch Operations
- `SYSCALL_WATCH_REGISTER` (0x06): Register a watch for graph changes
- `SYSCALL_WATCH_POLL` (0x07): Poll for changes from a registered watch

### Device Access
- `SYSCALL_KBD_READ` (0x08): Read keyboard scancodes
- `SYSCALL_FB_INFO` (0x09): Get framebuffer information
- `SYSCALL_FB_MAP` (0x0A): Map framebuffer into userspace
- `SYSCALL_MOUSE_READ` (0x0C): Read mouse events

### Capability Management
- `SYSCALL_GRANT_CAPABILITY` (0x10): Grant a capability to another bundle

## Bundle and Capability Model

ThingOS uses a capability-based security model where:

1. **Bundles** represent units of authority. Each task runs within a bundle context.
2. **Ownership** is tracked via `OWNS` edges from bundle nodes to Things they own.
3. **Capabilities** are edges from bundle nodes to Things they can access:
   - *Data capabilities* (delegable by the Thing owner):
     - `CAN_READ`: Permission to read a Thing's properties
     - `CAN_WRITE`: Permission to modify a Thing's properties
     - `CAN_LINK`: Permission to create edges involving a Thing
   - *Hardware capabilities* (minted by the kernel only):
     - `CAN_HANDLE_IRQ`: Permission to service an interrupt source
     - `CAN_DMA`: Permission to set up DMA transfers
     - `CAN_MMIO`: Permission to perform MMIO operations
     - `CAN_PORT_IO`: Permission to perform port I/O

### Granting Capabilities

Bundles can delegate their capabilities to other bundles using
`SYSCALL_GRANT_CAPABILITY`.

- Data capabilities (`CAN_READ`, `CAN_WRITE`, `CAN_LINK`) may be granted by the
  owner of the target Thing (or by the kernel, which implicitly owns
  everything).
- Hardware capabilities (`CAN_HANDLE_IRQ`, `CAN_DMA`, `CAN_MMIO`,
  `CAN_PORT_IO`) can only be granted by the kernel.

```rust
// Example: Grant read access to another bundle
use userland::{canon, grant_capability};

// Grant read permission on thing_id to other_bundle_id
let success = grant_capability(other_bundle_id, thing_id, canon::CAN_READ);
if success {
    // Capability was successfully granted
}
```

Alternatively, using the low-level API:

```rust
use userland::{canon, GrantCapabilityRequest};
use userland::sys::grant_capability_raw;

let request = GrantCapabilityRequest {
    grantee: other_bundle_id,
    target: thing_id,
    capability: canon::CAN_READ,
};
let payload = postcard::to_allocvec(&request).unwrap();
grant_capability_raw(&payload);
```

## Future Directions

- Schema definitions as Things (representing Kinds and Predicates)
- Graph queries and indices (finding Things by field values, traversing edges)
- Persistent journal and graph storage
- Graph diff and merge for distributed scenarios
