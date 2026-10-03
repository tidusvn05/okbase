//! Round-trip and edit tests against the real bundles in `fixtures/`.

use std::path::{Path, PathBuf};

use okbase_core::{Concept, FrontmatterState, Severity, discover, validate};
use serde_json::json;

fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../fixtures")
        .join(name)
}

const BUNDLES: [&str; 5] = [
    "okf-official/acme_retail",
    "okf-official/crypto_bitcoin",
    "okf-official/ga4",
    "okf-official/stackoverflow",
    "openclaw-s",
];

fn docs(bundle: &str) -> Vec<(Concept, String)> {
    let root = fixture(bundle);
    let paths = discover(&root).unwrap();
    assert!(!paths.is_empty(), "{bundle} has no documents");
    paths
        .iter()
        .map(|p| {
            (
                Concept::read(&root, p).unwrap(),
                std::fs::read_to_string(root.join(p)).unwrap(),
            )
        })
        .collect()
}

#[test]
fn round_trip_is_byte_identical() {
    for bundle in BUNDLES {
        for (doc, text) in docs(bundle) {
            assert_eq!(doc.render(), text, "{bundle}/{}", doc.path);
        }
    }
}

/// Lines outside the edited key's block must be untouched: the old and new texts
/// share a prefix and a suffix that together cover every line but the key's own.
fn assert_only_key_changed(before: &str, after: &str, key: &str, ctx: &str) {
    let b: Vec<&str> = before.split_inclusive('\n').collect();
    let a: Vec<&str> = after.split_inclusive('\n').collect();
    let prefix = b.iter().zip(&a).take_while(|(x, y)| x == y).count();
    let suffix = b[prefix..]
        .iter()
        .rev()
        .zip(a[prefix..].iter().rev())
        .take_while(|(x, y)| x == y)
        .count();
    let changed_old = &b[prefix..b.len() - suffix];
    let changed_new = &a[prefix..a.len() - suffix];
    for line in changed_old.iter().chain(changed_new) {
        let top_level = !line.starts_with([' ', '\t', '#', '-', '\r', '\n']);
        if top_level {
            assert!(
                line.starts_with(key)
                    || line.starts_with(&format!("\"{key}"))
                    || line.starts_with(&format!("'{key}")),
                "{ctx}: edit of `{key}` touched line {line:?}"
            );
        }
    }
    assert!(
        !changed_new.is_empty() || !changed_old.is_empty(),
        "{ctx}: nothing changed"
    );
}

#[test]
fn setting_each_key_changes_only_that_key() {
    let mut edited = 0;
    for bundle in BUNDLES {
        for (doc, text) in docs(bundle) {
            if doc.frontmatter.state() != FrontmatterState::Valid {
                continue;
            }
            let keys: Vec<String> = doc.frontmatter.keys().map(str::to_owned).collect();
            for key in keys.iter().chain([&"okbase_new".to_owned()]) {
                let ctx = format!("{bundle}/{}", doc.path);
                for value in [
                    json!("changed value"),
                    json!(["a", "b c"]),
                    json!({"by": "human:x", "n": 1}),
                ] {
                    let mut d = doc.clone();
                    d.frontmatter
                        .set(key, value.clone())
                        .unwrap_or_else(|e| panic!("{ctx}: set {key}: {e}"));
                    let out = d.render();
                    assert!(out.ends_with(&doc.body), "{ctx}: body changed");
                    assert_only_key_changed(&text, &out, key, &ctx);
                    let re = Concept::parse(Path::new(&doc.path), &out).unwrap();
                    assert_eq!(re.frontmatter.get(key), Some(&value), "{ctx}");
                    for other in keys.iter().filter(|k| *k != key) {
                        assert_eq!(
                            re.frontmatter.get(other),
                            doc.frontmatter.get(other),
                            "{ctx}: {other}"
                        );
                    }
                    edited += 1;
                }
                let mut d = doc.clone();
                if d.frontmatter.remove(key).unwrap() {
                    assert_only_key_changed(
                        &text,
                        &d.render(),
                        key,
                        &format!("{bundle}/{}", doc.path),
                    );
                }
            }
        }
    }
    assert!(edited > 500, "only {edited} edits exercised");
}

#[test]
fn official_bundles_conform_and_openclaw_does_not() {
    for bundle in &BUNDLES[..4] {
        for (doc, _) in docs(bundle) {
            let errors: Vec<_> = validate(&doc)
                .into_iter()
                .filter(|i| i.severity == Severity::Error)
                .collect();
            assert!(errors.is_empty(), "{bundle}/{}: {errors:?}", doc.path);
        }
    }
    let failing = docs("openclaw-s")
        .iter()
        .filter(|(d, _)| !validate(d).is_empty())
        .count();
    assert!(
        failing > 20,
        "expected most OpenClaw pages to miss `type`, got {failing}"
    );
}
