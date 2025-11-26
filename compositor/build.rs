use flate2::read::GzDecoder;
use std::env;
use std::fs::{self, File};
use std::io::{self, Read};
use std::path::{Path, PathBuf};
use std::process::Command;

const UNIFONT_URL: &str =
    "https://unifoundry.com/pub/unifont/unifont-15.1.05/font-builds/unifont-15.1.05.hex.gz";
const UNIFONT_GZ: &str = "unifont-15.1.05.hex.gz";
const UNIFONT_HEX: &str = "unifont-15.1.05.hex";

fn main() {
    let manifest_dir = env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR not set");
    println!("cargo:rustc-link-arg=-T{}/link.ld", manifest_dir);
    println!("cargo:rerun-if-changed=link.ld");

    let out_dir = PathBuf::from(env::var("OUT_DIR").expect("OUT_DIR not set"));
    let gz_path = out_dir.join(UNIFONT_GZ);
    let hex_path = out_dir.join(UNIFONT_HEX);

    if !gz_path.exists() {
        fetch_font(&gz_path).expect("Failed to download Unifont");
    }

    if !hex_path.exists() {
        decompress_font(&gz_path, &hex_path).expect("Failed to decompress Unifont");
    }

    println!("cargo:rerun-if-changed=build.rs");
}

fn fetch_font(dest: &Path) -> io::Result<()> {
    let status = Command::new("curl")
        .args(["-L", UNIFONT_URL, "-o"])
        .arg(dest)
        .status()?;
    if !status.success() {
        return Err(io::Error::new(
            io::ErrorKind::Other,
            "curl failed fetching Unifont",
        ));
    }
    Ok(())
}

fn decompress_font(src: &Path, dest: &Path) -> io::Result<()> {
    let input = File::open(src)?;
    let mut decoder = GzDecoder::new(input);
    let mut buf = Vec::new();
    decoder.read_to_end(&mut buf)?;
    fs::write(dest, &buf)?;
    Ok(())
}
