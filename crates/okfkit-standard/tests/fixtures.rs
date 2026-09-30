//! Level assessment and field mapping on the bundles in `fixtures/`.

use std::path::{Path, PathBuf};

use okfkit_core::{Concept, discover};
use okfkit_standard::{Level, VOCABULARY_PATH, assess_bundle, meta};

fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../fixtures")
        .join(name)
}

#[test]
fn official_bundles_are_l1() {
    for b in ["acme_retail", "crypto_bitcoin", "ga4", "stackoverflow"] {
        let a = assess_bundle(&fixture(&format!("okf-official/{b}")), VOCABULARY_PATH).unwrap();
        assert_eq!(
            a.level,
            Some(Level::L1),
            "{b}: {:#?}",
            a.blocking(Level::L1).collect::<Vec<_>>()
        );
        assert!(
            a.blocking(Level::L2).any(|f| f.code == "no-vocabulary"),
            "{b}"
        );
    }
}

#[test]
fn openclaw_fails_l0_but_reads_through_mapping() {
    let root = fixture("openclaw-s");
    let a = assess_bundle(&root, VOCABULARY_PATH).unwrap();
    assert_eq!(a.level, None);
    assert!(a.blocking(Level::L0).all(|f| f.code == "missing-type"));
    assert!(
        a.findings
            .iter()
            .any(|f| f.code == "missing-description" && f.message.contains("found `summary`"))
    );

    let mut mapped = 0;
    for rel in discover(&root).unwrap() {
        let doc = Concept::read(&root, &rel).unwrap();
        if doc.is_reserved() {
            continue;
        }
        let m = meta(&doc.frontmatter);
        assert!(m.title.is_some(), "{}: no title", doc.path);
        let d = m
            .description
            .unwrap_or_else(|| panic!("{}: no description", doc.path));
        mapped += usize::from(d.from == "summary");
    }
    assert!(
        mapped >= 25,
        "only {mapped} descriptions came from `summary`"
    );
}
