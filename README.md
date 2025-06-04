# ThingOS

An experimental Rust operating system. It is currently a small kernel that boots with the Limine bootloader, sets up basic x86\_64 hardware, and runs a toy user program. The long term idea is to represent running state as a graph of `Thing`s and `Fact`s, but persistence and advanced services are still to come.

## Building and running

The top level `GNUmakefile` builds an ISO image and runs it under QEMU.

```bash
# Build kernel and example program
make

# Boot the image in QEMU
make run
```

`KARCH` can be set to `x86_64` (default) or other architectures supported by the Makefile such as `aarch64` and `riscv64`.

## Code overview

- **kernel/** – the Rust kernel crate. `system.rs` performs initialization:
  - sets up the GDT, paging, kernel stack and heap
  - enables the syscall mechanism and interrupt handling
  - bootstraps a small `Graph` with built in kinds and facts
  - initializes the framebuffer and PS/2 devices
  - creates an HPET/RTC based `Clock`
  - loads the `hello_from` ELF binary as a user task

  After these steps the kernel enables interrupts and halts waiting for events. A basic scheduler exists but is not yet used.

- **hello\_from/** – minimal userland program that prints text using a syscall.
- **thing\_macros/** – proc macros used by the kernel.

## Status

This repository is in a very early stage. Persistence, multitasking and higher level services are not implemented yet. Development is focused on getting the core kernel up and experimenting with the graph data model.

## License

MIT
