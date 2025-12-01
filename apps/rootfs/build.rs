use std::env;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::process::Command;

const FONT_ITEMS: &[(&str, &str)] = &[
    (
        "https://raw.githubusercontent.com/googlefonts/noto-fonts/main/hinted/ttf/NotoSansSymbols/NotoSansSymbols-Regular.ttf",
        "NotoSansSymbols-Regular.ttf",
    ),
    (
        "https://raw.githubusercontent.com/googlefonts/noto-fonts/main/hinted/ttf/NotoSansSymbols2/NotoSansSymbols2-Regular.ttf",
        "NotoSansSymbols2-Regular.ttf",
    ),
];

fn main() {
    let manifest_dir =
        PathBuf::from(env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR not set"));
    let out_dir = PathBuf::from(env::var("OUT_DIR").expect("OUT_DIR not set"));
    let repo_root = manifest_dir
        .parent()
        .and_then(|p| p.parent())
        .expect("Failed to locate repository root");
    let cache_dir = repo_root.join("fonts-cache");
    fs::create_dir_all(&cache_dir).expect("Unable to create font cache directory");

    for (url, file_name) in FONT_ITEMS {
        let cache_path = cache_dir.join(file_name);
        if !cache_path.exists() {
            fetch_font(&cache_path, url).expect("Failed to download font");
        }

        let out_path = out_dir.join(file_name);
        fs::copy(&cache_path, &out_path).expect("Failed to copy font to OUT_DIR");
        println!("cargo:rerun-if-changed={}", cache_path.display());
    }

    println!("cargo:rerun-if-changed=build.rs");
}

fn fetch_font(dest: &Path, url: &str) -> io::Result<()> {
    let status = Command::new("curl")
        .args(["-L", url, "-o"])
        .arg(dest)
        .status()?;

    if !status.success() {
        return Err(io::Error::new(
            io::ErrorKind::Other,
            "curl failed to download font",
        ));
    }

    Ok(())
}
