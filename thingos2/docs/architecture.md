# ThingOS v2 Architecture

ThingOS v2 treats the property graph as the *only* API. Bundles do everything by
creating nodes, linking edges, and reacting to watch events that mirror graph
mutations. The kernel hosts the authoritative graph, enforces capabilities, and
schedules bundle tasks on top of architecture-specific bring-up (Limine on
x86_64 for now).

## Layout

- `core/` – Pure graph engine that models nodes, edges, capabilities, and
  watches. There are no hardware assumptions in this crate; it is meant to be
  embedded by the kernel.
- `kernel/` – In-kernel orchestration layer. It embeds the graph, exposes the
  syscall surface (`fiat_node`, `link`, `get_nodes`, `get_props`, `set_props`,
  `watch_register`, `watch_poll`), and schedules bundle tasks.
- `bundles/` – Graph-native userland bundles. Drivers, compositor, and apps
  publish their state into the graph and listen via watches. Example drivers and
  compositor scaffolding live here.
- `arch/x86_64/` – Platform bring-up for Limine boot on x86_64. It wires the
  architecture into the kernel scheduler and launches initial bundles.
- `docs/` – Human-facing documentation of the vision and the moving parts.

## Graph Model

### Nodes and edges

Nodes are opaque `NodeId` handles with multiple labels and property maps. Edges
connect two nodes, carry a `kind` string, and may also have properties. Every
node records the bundle that created it. The kernel automatically creates an
`owns` edge from the bundle node to each resource it allocates.

### Capabilities

Capabilities are expressed as edges emanating from bundle nodes:

- `(:Bundle)-[:CAN_READ]->(resource)` reveals nodes to that bundle.
- `(:Bundle)-[:CAN_WRITE]->(resource)` allows property mutation.
- `(:Bundle)-[:CAN_LINK { kind: "HAS_SURFACE", target: <to> }]->(from)` permits
  creating specific edge kinds from `from` to `to` even without ownership.

The `core` graph enforces these checks for every mutation and query.

### Queries and watches

Bundles read the world using structured `NodePattern` queries (labels and
property equality). Watches are persistent subscriptions registered in the
kernel. The kernel computes which watches match on node creation, property
mutation, or edge creation and queues `WatchEvent`s for the owning bundle.

## Syscalls

The kernel exposes graph operations plus minimal scheduling/time primitives:

- `sys_graph_fiat_node(bundle, labels, props) -> NodeId`
- `sys_graph_link(bundle, kind, from, to, props) -> EdgeId`
- `sys_graph_get_nodes(bundle, pattern) -> Vec<NodeId>`
- `sys_graph_get_props(bundle, node, keys) -> Map`
- `sys_graph_set_props(bundle, node, props)`
- `sys_watch_register(bundle, pattern, watch_id)`
- `sys_watch_poll(bundle) -> Vec<WatchEvent>`
- `sys_yield`, `sys_sleep`, `sys_time_now` (scheduling and time primitives)

Everything else (IPC, notifications, resource discovery) is expressed by
modifying or watching the graph.

## Bundles and Ownership

Every executable is a bundle represented by a `:Bundle` node. When a bundle
creates resources, the kernel tags them with the bundle owner and adds an
`owns` edge. Bundles may create edges from nodes they own or edges pointing
into nodes they own. Cross-bundle mutations require explicit capability edges.

## Drivers and Compositor

- **Framebuffer driver**: publishes `:Framebuffer` and `:Surface` nodes,
  representing the physical framebuffer and shared surfaces. It ultimately maps
  shared memory for surface pixels and flushes to hardware.
- **Input drivers**: publish `:Keyboard`/`:Mouse` device nodes and create event
  nodes (`:KeyEvent`, `:MouseMove`, `:MouseButton`).
- **Compositor**: watches `:Window` and `:Surface` patterns, links surfaces into
  the framebuffer, and renders the cursor. It is unprivileged and only works via
  graph capabilities.

## Property Endpoints

Bundles can expose `:PropertyEndpoint` nodes to interpose on property access.
Other bundles create `:PropertyRequest` nodes to ask the owning bundle to read
or mutate properties. Responses are posted as `:PropertyResponse` nodes.

## Shared Memory and IPC

Shared buffers are modeled as graph nodes (`:SharedBuffer { size, usage }`).
Hardware drivers own the buffers and share them with consumers through
capability edges. Synchronization happens through graph events and watches—there
are no side channels.

## Boot Flow

1. Limine loads the `thingos2` kernel binary and transfers control to
   `arch/x86_64` bring-up code.
2. Architecture code initializes GDT/IDT/TSS, paging, and timers, then hands the
   CPU to `thingos2-kernel`.
3. Kernel constructs the graph, registers the bundle nodes, and spawns driver
   and compositor bundles as real tasks.
4. Drivers publish hardware state into the graph; the compositor watches and
   renders surfaces.
