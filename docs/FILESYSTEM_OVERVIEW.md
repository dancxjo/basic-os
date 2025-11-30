# Filesystem Overview

The ThingOS filesystem is a graph-backed directory tree managed by the `rootfs` bundle.
It provides a familiar hierarchical view (directories, files, devices) over the underlying graph of Things.

## Structure

- `/`: Root directory.
- `/bin`: Contains executable bundles.
- `/dev`: Contains device nodes.
- `/tmp`: Temporary storage.

## Path Resolution

The `userland::fs` module provides `lookup_path` and `read_dir` functions to traverse the filesystem.
Path resolution works by following `contains` edges from parent to child, matching the `NAME` property.
Nodes also have a `PARENT` property pointing back to their directory, which allows efficient enumeration of directory contents.

## /bin and Executables

Entries in `/bin` represent executable bundles. They are of kind `FILE` and contain a `BUNDLE_ID` property (currently a placeholder derived from the name).
This allows a launcher to list available applications and execute them.

## /dev/tty0

`/dev/tty0` is a character device node bound to the "console" driver.
A future console bundle will attach to this device to provide terminal I/O (to QEMU serial, a desktop terminal window, or both).
It has the following properties:
- `KIND`: `DEVICE`
- `DEVICE_DRIVER`: "console"
- `DEVICE_ID`: "tty0"
