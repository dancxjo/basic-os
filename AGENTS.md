# AGENTS Instructions

## Formatting
- Use four spaces for indentation in all Rust files.
- Run `cargo fmt --all` in each crate before committing changes.
- If `cargo fmt` complains that `rustfmt` is missing, install it for your
  host toolchain. For example with the default toolchain run
  `rustup component add rustfmt`. If you're using nightly explicitly use
  `rustup component add --toolchain nightly-x86_64-unknown-linux-gnu rustfmt`.

## Programmatic checks
- Build the OS image by running `make` from the repository root. This fetches
  the Limine bootloader and compiles the Rust kernel and user program.
- Ensure the `xorriso` utility is installed (`apt-get install xorriso` on
  Debian-based systems). Without it `make` will fail when generating the ISO
  image.

- Make sure the `x86_64-unknown-none` target is installed:
  `rustup target add x86_64-unknown-none`.

## Build notes
- The kernel crate is a binary. Its entry point is `kernel/src/main.rs` which
  exposes the `kmain` function. There is no `lib.rs` in this crate.
- Run `cargo` commands inside the individual crate directories such as
  `kernel/` or `hello_from/`. The top-level `make` command
  orchestrates building all crates and creating the bootable image.

## Pull request guidelines
- Summarize user visible changes.
- Mention whether `make` succeeded or failed in the testing section.
