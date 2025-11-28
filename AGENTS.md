# AGENTS Instructions

## Formatting
- Use four spaces for indentation in all Rust files.
- Run `cargo fmt --all` inside each crate (`kernel/`, `compositor/`, `apps/*/`, and `userland/`) before committing changes.
- If `cargo fmt` complains that `rustfmt` is missing install it for your host toolchain, e.g. `rustup component add rustfmt` or `rustup component add --toolchain nightly-x86_64-unknown-linux-gnu rustfmt`.

## Programmatic checks
- Build the OS image by running `make` from the repository root.  This downloads the Limine bootloader and builds the kernel plus the userland compositor bundle.
- Ensure the `xorriso` utility is installed (`apt-get install xorriso` on Debian based systems) otherwise the ISO creation will fail.
- Install the `x86_64-unknown-none` target with `rustup target add x86_64-unknown-none`.

## Building and running
- The provided `GNUmakefile` drives the build.  `make` builds the kernel and userland runner/compositor and produces an ISO image.
- Run `make run` to boot the ISO in QEMU.  The variable `KARCH` selects the architecture (default `x86_64`).
- Run `make clean` to remove build artifacts.

## Repository layout
- `kernel/` – Rust kernel crate.  `kmain` in `src/main.rs` is the entry point.
- `compositor/` – userland compositor binary and library.
- `userland/` – shared userland support library (syscalls/graph helpers).
- `apps/` – small user apps (clouds, hello, clock) that publish window buffers.
- `limine.conf` describes boot configuration and modules included in the ISO.

## Pull request guidelines
- Summarize user visible changes in the PR description.
- Mention whether `make` succeeded or failed in the testing section.
- Organize your commits meaningfully, with excellent but succinct commit messages.
