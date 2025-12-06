use std::env;
use std::fs::File;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::process::Command;

// Noto Fonts
const NOTO_SANS_URL: &str = "https://github.com/notofonts/noto-fonts/raw/refs/heads/main/hinted/ttf/NotoSans/NotoSans-Regular.ttf";
const NOTO_SANS_SYMBOLS_URL: &str = "https://github.com/notofonts/noto-fonts/raw/refs/heads/main/hinted/ttf/NotoSansSymbols/NotoSansSymbols-Regular.ttf";
const NOTO_SANS_SYMBOLS2_URL: &str = "https://github.com/notofonts/noto-fonts/raw/refs/heads/main/hinted/ttf/NotoSansSymbols2/NotoSansSymbols2-Regular.ttf";

fn main() {
    let target = env::var("TARGET").expect("TARGET not set");
    if !target.contains("linux") {
        let manifest_dir = env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR not set");
        println!("cargo:rustc-link-arg=-T{}/link.ld", manifest_dir);
        println!("cargo:rerun-if-changed=link.ld");
    }

    let out_dir = PathBuf::from(env::var("OUT_DIR").expect("OUT_DIR not set"));

    // Download Noto fonts
    let fonts = [
        ("NotoSans-Regular.ttf", NOTO_SANS_URL),
        ("NotoSansSymbols-Regular.ttf", NOTO_SANS_SYMBOLS_URL),
        ("NotoSansSymbols2-Regular.ttf", NOTO_SANS_SYMBOLS2_URL),
    ];

    for (filename, url) in fonts {
        let dest = out_dir.join(filename);
        if !dest.exists() {
            fetch_url(url, &dest).expect(&format!("Failed to download {}", filename));
        }
    }

    // Generate fonts.rs to include them
    let fonts_rs = out_dir.join("fonts_includes.rs");
    let mut f = File::create(&fonts_rs).expect("Failed to create fonts_includes.rs");
    
    writeln!(f, "pub const NOTO_SANS: &[u8] = include_bytes!({:?});", out_dir.join("NotoSans-Regular.ttf")).unwrap();
    writeln!(f, "pub const NOTO_SANS_SYMBOLS: &[u8] = include_bytes!({:?});", out_dir.join("NotoSansSymbols-Regular.ttf")).unwrap();
    writeln!(f, "pub const NOTO_SANS_SYMBOLS2: &[u8] = include_bytes!({:?});", out_dir.join("NotoSansSymbols2-Regular.ttf")).unwrap();

    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rustc-env=OUT_DIR={}", out_dir.display());
}

fn fetch_url(url: &str, dest: &Path) -> io::Result<()> {
    // Retry a few times
    for _ in 0..3 {
        let status = Command::new("curl")
            .args(["-L", url, "-o"])
            .arg(dest)
            .status()?;
        if status.success() {
            return Ok(());
        }
    }
    Err(io::Error::new(
        io::ErrorKind::Other,
        format!("curl failed fetching {}", url),
    ))
}
