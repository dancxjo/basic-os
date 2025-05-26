// build.rs
fn main() {
    println!("cargo:rustc-cdylib-link-arg=--no-entry");
}
