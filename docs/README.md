# Design Documents

This directory collects design notes and reference material for the project. Use it to capture architecture decisions and future plans rather than source code or tooling scripts. The journal is a kernel-internal mechanism; userland observes the system through graph queries and watches rather than raw journal access.

## Documents
- **GRAPH_API.md** – outlines the Thing-based graph model, syscall surface, and host vs. kernel behavior.
- **ABI_CONTRACT.md** – documents the guarantees and non-guarantees for the kernel and host runtimes.
- **THINGOS_VISION.md** – summarizes the long-term goals and guiding principles for the OS.
