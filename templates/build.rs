//! Compile the declarative template catalog into offline assets.

use std::collections::BTreeSet;
use std::path::{Component, Path};

use serde::Deserialize;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Catalog {
    format_version: u32,
    templates: Vec<Entry>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Entry {
    path: String,
}

fn collect(root: &Path, dir: &Path, output: &mut String) {
    println!("cargo:rerun-if-changed={}", dir.display());
    let mut entries: Vec<_> = std::fs::read_dir(dir)
        .unwrap()
        .map(Result::unwrap)
        .collect();
    entries.sort_by_key(|e| e.file_name());
    for entry in entries {
        let path = entry.path();
        let ty = entry.file_type().unwrap();
        assert!(!ty.is_symlink(), "template symlinks are unsupported");
        if ty.is_dir() {
            collect(root, &path, output);
        } else {
            assert!(ty.is_file(), "template assets must be regular files");
            let name = path
                .strip_prefix(root)
                .unwrap()
                .to_str()
                .unwrap()
                .replace('\\', "/");
            // Embed literal source text: the package is self-contained on crates.io too.
            let content = std::fs::read_to_string(&path).unwrap();
            output.push_str(&format!("({name:?}, {content:?}),\n"));
        }
    }
}

fn main() {
    println!("cargo:rerun-if-changed=catalog.yaml");
    let root = std::env::current_dir().unwrap();
    let catalog: Catalog =
        serde_saphyr::from_str(&std::fs::read_to_string("catalog.yaml").unwrap()).unwrap();
    assert_eq!(catalog.format_version, 1, "unsupported catalog format");
    let mut seen = BTreeSet::new();
    let mut output = String::from(
        "/// UTF-8 template files, keyed by catalog-relative path.\npub const FILES: &[(&str, &str)] = &[\n",
    );
    for entry in catalog.templates {
        assert!(!entry.path.is_empty() && !entry.path.contains('\\'));
        assert!(
            Path::new(&entry.path)
                .components()
                .all(|c| matches!(c, Component::Normal(_)))
        );
        assert!(seen.insert(entry.path.clone()), "duplicate catalog path");
        assert!(
            root.join(&entry.path).join("template.yaml").is_file(),
            "missing template manifest"
        );
        collect(&root, &root.join(entry.path), &mut output);
    }
    output.push_str("];\n");
    std::fs::write(
        Path::new(&std::env::var_os("OUT_DIR").unwrap()).join("assets.rs"),
        output,
    )
    .unwrap();
}
