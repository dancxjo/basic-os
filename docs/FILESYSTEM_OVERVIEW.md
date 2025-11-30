# Filesystem Overview

The ThingOS filesystem is a graph-backed directory tree managed by the `rootfs` bundle.
It provides a familiar hierarchical view (directories, files, devices) over the
underlying graph of Things, without treating “files” as the primary ontology.

`rootfs` is a userland service: the kernel does not hard-code directories or
executables. Instead, `rootfs` creates and maintains the filesystem structure
inside the graph.

## Structure

The initial filesystem layout is:

- `/`  
  Root directory of the filesystem.

- `/bin`  
  Contains executable bundles. This is the **canonical registry of available
  userland programs**. Both `init` and the desktop/launcher should discover
  programs here, rather than hard-coding any list.

- `/dev`  
  Contains device nodes that represent character and block devices (for now,
  primarily character devices such as `tty0`).

- `/tmp`  
  Temporary storage. Semantics may evolve, but it is intended for scratch
  data that does not need to persist across boots.

## Node Kinds and Graph Representation

Filesystem nodes are represented as Things with a small, explicit kind:

- `DIR`    — Directory
- `FILE`   — Regular file or executable
- `DEVICE` — Device node (e.g., character device)

Each node has a `NAME` property (its basename) and participates in a directory
tree modeled via graph relationships:

- `(:FsNode { KIND: "DIR" })-[:CONTAINS { NAME: "child_name" }]->(:FsNode ...)`  
  Parent directory contains a child entry with the given name.

- `(:FsNode)-[:PARENT]->(:FsNode { KIND: "DIR" })`  
  Child node points back to its parent directory.

`CONTAINS` edges and the `NAME` property are used for path traversal. The
`PARENT` relationship can be used for reverse walks and efficient directory
enumeration.

Additional per-node properties:

- `KIND`: `"DIR" | "FILE" | "DEVICE"`
- `MODE` (optional): Permissions / mode bits (reserved for future use)
- `OWNER_BUNDLE` (optional): Owning bundle, if applicable

## Path Resolution

The `userland::fs` module provides `lookup_path` and `read_dir` functions to
traverse the filesystem from userland.

### `lookup_path`

- Accepts an absolute path such as `/`, `/bin`, `/bin/text_editor`,
  `/dev/tty0`, etc.
- Splits the path into components and walks the tree from `/` by following
  `CONTAINS` edges and matching `NAME`.
- Returns a structured `FsNode` description on success, or an error if the path
  does not exist.

### `read_dir`

- Accepts an absolute path to a directory (e.g. `/bin` or `/dev`).
- Resolves it to a directory node using `lookup_path`.
- Enumerates its children via `CONTAINS` edges and returns a list of `FsNode`
  entries, typically sorted by `NAME`.

These functions are the primary way for other bundles (such as `init` or the
desktop) to discover what exists in the filesystem.

## /bin and Executables

Entries in `/bin` represent executable bundles. They have:

- `KIND`: `FILE`
- `NAME`: logical app name (e.g. `compositor`, `graph_demo`, `text_editor`)
- `BUNDLE_ID` or `BIN_NAME`: identifies which bundle/binary to spawn

`rootfs` populates `/bin` based on a shared list of applications (an
"apps manifest"), so that there is a single source of truth for which programs
are shipped with the system image.

Future/optional metadata on `/bin` entries:

- `AUTOSTART`: `true | false`  
  Indicates whether `init` should automatically launch this program at startup.

- `SHOW_IN_LAUNCHER`: `true | false`  
  Indicates whether the desktop/launcher should present this entry to the user
  as an application icon or menu item.

### Launching from /bin

- **Init**: Instead of hard-coding each program, `init` should:
  - Call `read_dir("/bin")`.
  - Filter entries where `KIND = FILE` and `AUTOSTART = true`.
  - Spawn the corresponding bundles using `BUNDLE_ID`/`BIN_NAME`.

- **Desktop / launcher**: To list available applications, the desktop bundle can:
  - Call `read_dir("/bin")`.
  - Filter entries based on `SHOW_IN_LAUNCHER`.
  - Render icons / menu items for those entries.
  - Launch them via the same bundle/binary identifiers.

In this way, `/bin` serves as the canonical registry of installed programs for
both system startup and user-facing UI.

## /dev/tty0

`/dev/tty0` is a character device node bound to the `"console"` driver. It is
represented as an `FsNode` under `/dev` with:

- `KIND`: `DEVICE`
- `NAME`: `tty0`
- `DEVICE_DRIVER`: `"console"`
- `DEVICE_ID`: `"tty0"`

A future `console` bundle will attach to this device to provide terminal I/O:

- It may forward output to:
  - QEMU serial,
  - a desktop terminal window,
  - or both.

- It may feed input from:
  - Keyboard events,
  - other input sources,
  - or a combination.

From the perspective of other bundles, `/dev/tty0` will behave like a standard
character device: a readable/writable stream endpoint that represents the
primary console.
