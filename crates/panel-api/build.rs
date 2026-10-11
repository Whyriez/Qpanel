use std::fs;
use std::path::Path;

fn main() {
    // Ensure web/dist exists with a fallback index.html for RustEmbed
    // when building in headless CI or without prior frontend build.
    let dist_dir = Path::new("../../web/dist");
    if !dist_dir.exists() {
        let _ = fs::create_dir_all(dist_dir);
    }
    let index_file = dist_dir.join("index.html");
    if !index_file.exists() {
        let _ = fs::write(
            index_file,
            "<!DOCTYPE html><html><head><title>Nusa Panel</title></head><body><div id=\"root\"></div></body></html>",
        );
    }
    println!("cargo:rerun-if-changed=../../web/dist");
}
