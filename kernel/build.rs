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
}
