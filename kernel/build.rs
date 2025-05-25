fn main() {
    let arch = std::env::var("CARGO_CFG_TARGET_ARCH").unwrap();

    // Tell cargo to pass the linker script to the linker
    println!("cargo:rustc-link-arg=-Tlinker-{arch}.ld");

    // Re-run if the linker script changes
    println!("cargo:rerun-if-changed=linker-{arch}.ld");
}
