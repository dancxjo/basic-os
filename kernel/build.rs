use std::{env, path::PathBuf};

use cc::Build;

fn main() {
    let arch = std::env::var("CARGO_CFG_TARGET_ARCH").unwrap();

    // Tell cargo to pass the linker script to the linker
    println!("cargo:rustc-link-arg=-Tlinker-{arch}.ld");

    // Re-run if the linker script changes
    println!("cargo:rerun-if-changed=linker-{arch}.ld");

    let _out_dir = PathBuf::from(env::var("OUT_DIR").unwrap());

    // Compile syscall_entry.S using cc
    Build::new()
        .file("src/arch/x86_64/asm/syscall_entry.S")
        .file("src/arch/x86_64/asm/restore_context.S")
        .file("src/arch/x86_64/asm/tick_handler.S")
        .compile("asm_routines");
}
