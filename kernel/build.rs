use std::{env, path::PathBuf, process::Command};

use cc::Build;

fn main() {
    let arch = std::env::var("CARGO_CFG_TARGET_ARCH").unwrap();

    // Tell cargo to pass the linker script to the linker
    println!("cargo:rustc-link-arg=-Tlinker-{arch}.ld");

    // Re-run if the linker script changes
    println!("cargo:rerun-if-changed=linker-{arch}.ld");

    let out_dir = PathBuf::from(env::var("OUT_DIR").unwrap());

    // Compile syscall_entry.S using cc
    Build::new()
        .file("src/syscall_entry.S")
        .compile("syscall_entry");
}
