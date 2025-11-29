# Copilot Instructions

## Repository Overview

ThingOS is an experimental Rust operating system. It boots with the Limine bootloader, sets up basic x86_64 hardware, and runs a toy user program. The project is in early development focused on bringing up the core kernel.

## Project Structure

- `kernel/` – Rust kernel crate. `kmain` in `src/main.rs` is the entry point.
- `hello_from/` – Minimal userland program that prints text using a syscall.
- `limine.conf` – Boot configuration and modules included in the ISO.
- `GNUmakefile` – Drives the build process.

## Build and Test

### Prerequisites

- Install the `x86_64-unknown-none` target: `rustup target add x86_64-unknown-none`
- Install `xorriso`: `apt-get install xorriso` (Debian-based systems)
- Install `rustfmt` if missing: `rustup component add rustfmt`

### Build Commands

```bash
# Build the OS image (downloads Limine bootloader and builds both Rust crates)
make

# Boot the ISO in QEMU
make run

# Clean build artifacts
make clean
```

The variable `KARCH` selects the architecture (default `x86_64`).

## Coding Standards

### Formatting

- Use four spaces for indentation in all Rust files.
- Run `cargo fmt --all` inside each crate (`kernel/` and `hello_from/`) before committing changes.

### Rustfmt Installation

- If `cargo fmt` complains that `rustfmt` is missing, install it: `rustup component add rustfmt` or `rustup component add --toolchain nightly-x86_64-unknown-linux-gnu rustfmt`.

## Pull Request Guidelines

- Summarize user-visible changes in the PR description.
- Mention whether `make` succeeded or failed in the testing section.
- Organize commits meaningfully with excellent but succinct commit messages.

## Validation

- Always run `make` from the repository root to verify builds succeed.
- Test changes with `make run` when possible (requires QEMU).

Note: When running `make run`, prefer running it with a timeout so QEMU exits automatically and you don't need to manually close or kill the QEMU window. Example: `timeout 300 make run`.
