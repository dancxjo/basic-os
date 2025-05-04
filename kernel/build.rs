use std::{fs, process::Command};

fn main() {
    let arch = std::env::var("CARGO_CFG_TARGET_ARCH").unwrap();

    // Tell cargo to pass the linker script to the linker
    println!("cargo:rustc-link-arg=-Tlinker-{arch}.ld");

    // Re-run if the linker script changes
    println!("cargo:rerun-if-changed=linker-{arch}.ld");

    // Compile and link the context switch assembly file
    cc::Build::new()
        .file("src/tick_handler.S")
        .compile("tick_handler");
    // Re-run if the assembly file changes
    println!("cargo:rerun-if-changed=src/tick_handler.S");

    // Compile and link the context switch assembly file
    cc::Build::new()
        .file("src/switch_to_task.S")
        .compile("switch_to_task");
    // Re-run if the assembly file changes
    println!("cargo:rerun-if-changed=src/switch_to_task.S");

    println!("cargo:rerun-if-changed=ap_trampoline.S");
    println!("cargo:rerun-if-changed=ap_trampoline.ld");

    // Output path
    let out_dir = env::var("OUT_DIR").unwrap();
    let output_bin = format!("{}/ap_trampoline.bin", out_dir);

    // Run the assembler
    let status = Command::new("nasm")
        .args(&["-f", "bin", "ap_trampoline.S", "-o", &output_bin])
        .status()
        .expect("Failed to assemble trampoline");
    assert!(status.success());

    // Copy to kernel image, or make available for include_bytes!
    fs::copy(&output_bin, "ap_trampoline.bin").unwrap();
}
