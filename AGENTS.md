# AGENTS Instructions

## Formatting
- Use four spaces for indentation in all Rust files.
- Run `cargo fmt --all` inside each crate (`kernel/` and `hello_from/`) before committing changes.
- If `cargo fmt` complains that `rustfmt` is missing install it for your host toolchain, e.g. `rustup component add rustfmt` or `rustup component add --toolchain nightly-x86_64-unknown-linux-gnu rustfmt`.

## Programmatic checks
- Build the OS image by running `make` from the repository root.  This downloads the Limine bootloader and builds both Rust crates.
- Ensure the `xorriso` utility is installed (`apt-get install xorriso` on Debian based systems) otherwise the ISO creation will fail.
- Install the `x86_64-unknown-none` target with `rustup target add x86_64-unknown-none`.

## Building and running
- The provided `GNUmakefile` drives the build.  `make` builds `kernel/` and `hello_from/` and produces an ISO image.
- Run `make run` to boot the ISO in QEMU.  The variable `KARCH` selects the architecture (default `x86_64`).
- Run `make clean` to remove build artifacts.

## Repository layout
- `kernel/` – Rust kernel crate.  `kmain` in `src/main.rs` is the entry point.
- `hello_from/` – tiny user program that prints a message via a syscall.
- `limine.conf` describes boot configuration and modules included in the ISO.

## Pull request guidelines
- Summarize user visible changes in the PR description.
- Mention whether `make` succeeded or failed in the testing section.
