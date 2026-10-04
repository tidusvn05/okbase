//! Embeds the built viewer (`web/dist/`, committed) as a table of `include_bytes!`,
//! so building okbase never needs Node.

use std::fmt::Write as _;
use std::path::{Path, PathBuf};

fn main() {
    let dist = Path::new(env!("CARGO_MANIFEST_DIR")).join("web/dist");
    println!("cargo:rerun-if-changed={}", dist.display());
    let mut files = Vec::new();
    if dist.is_dir() {
        walk(&dist, &mut files);
    }
    files.sort();
    let mut out = String::from("/// Built viewer files: (path relative to `web/dist`, bytes).\n");
    out.push_str("pub(crate) static ASSETS: &[(&str, &[u8])] = &[\n");
    for f in &files {
        let rel = f
            .strip_prefix(&dist)
            .expect("under dist")
            .to_string_lossy()
            .replace('\\', "/");
        writeln!(
            out,
            "    ({rel:?}, include_bytes!({:?})),",
            f.display().to_string()
        )
        .expect("write to String");
        println!("cargo:rerun-if-changed={}", f.display());
    }
    out.push_str("];\n");
    let dest = PathBuf::from(std::env::var("OUT_DIR").expect("OUT_DIR")).join("assets.rs");
    std::fs::write(dest, out).expect("write assets.rs");
}

fn walk(dir: &Path, out: &mut Vec<PathBuf>) {
    for entry in std::fs::read_dir(dir).expect("read web/dist") {
        let path = entry.expect("dir entry").path();
        if path.is_dir() {
            walk(&path, out);
        } else {
            out.push(path);
        }
    }
}
