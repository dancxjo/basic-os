# 🌱 ThingOS

> People, Places, Things, and Ideas

> A memory-first, graph-shaped operating system — born persistent,
> structurally typed, and ready to grow.

---

## ✨ What is ThingOS?

ThingOS is a graph-native operating system kernel.
It models memory, identity, and process state as a living network of **Things**.

Each Thing:
- Has a stable `UUID`
- Carries structured memory (`Bytes`, `Heap`, `Typed`)
- Belongs to a `Kind`
- Participates in relationships (`Facts`) with other Things

---

## 🧠 Core Concepts

### 🧱 Thing
A self-describing unit of memory. Think of it like a file, a process, a struct, or a page — all rolled into one.

```rust
Thing {
  uuid: Uuid,
  kind: "message",
  data: ThingData::Typed(*mut (), size),
}
```

### 🌿 Graph
All Things live in a `Graph`. It maintains:
- Things (nodes)
- Facts (edges)
- Kinds and Predicates (metadata)

UUIDs index the entire system — no names, just identity.

### 💾 Dirty Tracking
If a Thing is mutably borrowed, its UUID is automatically marked as dirty.
This forms the basis for **crash-proof persistence**: every change is journaled.

---

## ✅ Current Features

- [x] UUID-backed memory identity
- [x] Memory-safe struct insertion and access
- [x] Type-safe access via `as_typed()` / `as_typed_mut()`
- [x] Dirty tracking on mutable access
- [x] Live UUID → Thing lookup
- [x] Boot-time graph mutation and introspection
- [x] Serial logging of system state

---

## 🔜 Coming Next

- [ ] Journaled `Delta` logging for every memory mutation
- [ ] Bedrock: background persistence engine
- [ ] Rehydration of graph state from disk at boot
- [ ] Active relationships (e.g., allocator, scheduler)
- [ ] Graph-based GUI: The Garden

---

## 🔧 Example

```rust
let mut graph = Graph::new();
graph.insert("message", Message {
  text: "ThingOS\nPeople, places, things and ideas\n© 2025".to_owned(),
});

let msg = graph.find_mut(|t| t.kind == "message").unwrap();
let typed = msg.data.as_typed_mut::<Message>(msg.uuid).unwrap();
typed.text = "Mutation".to_owned();
```

Outputs:
```
Marked dirty: 18c97eac-b326-553a-b6d7-4c4621bf48bb
```

---

## 🌳 Philosophy

ThingOS is not just an OS — it's a living, breathing system where:
- Memory has structure
- Every allocation is an object
- Every object has meaning
- And every mutation has a story

---

## 🏗 Status

Early kernel stage — booting, allocating, tracking, and preparing to persist.

---

## 📜 License

MIT. All code copyright © 2025 the contributors.
