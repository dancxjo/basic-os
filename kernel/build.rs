use std::{env, fs, process::Command};

fn main() {
    let arch = env::var("CARGO_CFG_TARGET_ARCH").unwrap();
    println!("cargo:rustc-link-arg=-Tlinker-{arch}.ld");
    println!("cargo:rerun-if-changed=linker-{arch}.ld");

    for file in ["src/tick_handler.S", "src/switch_to_task.S"] {
        println!("cargo:rerun-if-changed={file}");
        let obj_name = std::path::Path::new(file)
            .file_stem()
            .unwrap()
            .to_str()
            .unwrap();

        cc::Build::new().file(file).flag("-m64").compile(obj_name);
    }

    // Assemble AP trampoline to ELF .o
    println!("cargo:rerun-if-changed=ap_trampoline.asm");
    let out_dir = env::var("OUT_DIR").unwrap();
    let trampoline_o = format!("{}/ap_trampoline.o", out_dir);
    let trampoline_bin = format!("{}/ap_trampoline.bin", out_dir);

    let status = Command::new("nasm")
        .args(&["-f", "elf32", "ap_trampoline.asm", "-o", &trampoline_o])
        .status()
        .expect("NASM failed");
    assert!(status.success());

    // Link to flat binary
    let status = Command::new("ld")
        .args(&[
            "-T",
            "ap_trampoline.ld",
            "-m",
            "elf_i386",
            "-nostdlib",
            "-o",
            &trampoline_bin,
            &trampoline_o,
        ])
        .status()
        .expect("ld failed");
    assert!(status.success());

    // Optional: copy to project root
    fs::copy(&trampoline_bin, "ap_trampoline.bin").unwrap();
}
