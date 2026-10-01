//! The planted problems in `fixtures/lint/` are all found, and nothing else.

use std::fs;
use std::path::{Path, PathBuf};

use okfkit_lint::{Level, LintConfig, Severity, fix_safe, lint};

fn fixture() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/lint")
}

#[derive(serde::Deserialize, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct Expected {
    path: String,
    line: usize,
    rule: String,
}

fn expected() -> Vec<Expected> {
    let mut e: Vec<Expected> =
        serde_json::from_str(&fs::read_to_string(fixture().join("expected.json")).unwrap())
            .unwrap();
    e.sort();
    e
}

#[test]
fn finds_exactly_the_planted_problems() {
    let report = lint(&fixture(), &LintConfig::level(Level::L3)).unwrap();
    let mut got: Vec<Expected> = report
        .diagnostics
        .iter()
        .map(|d| Expected {
            path: d.path.clone(),
            line: d.line.unwrap(),
            rule: d.rule.to_owned(),
        })
        .collect();
    got.sort();
    assert_eq!(got, expected());
    assert!(report.diagnostics.iter().all(|d| d.path != "good.md"));
    assert_eq!(report.level, None);
    // 21 distinct rules have a planted case; the rest (missing-title/tags/status/updated,
    // invalid-type/supersedes, meta-document errors, unreadable, doc-too-long) are unit-tested.
    let covered: std::collections::BTreeSet<_> =
        report.diagnostics.iter().map(|d| d.rule).collect();
    assert!(covered.len() >= 21, "{covered:?}");
}

#[test]
fn target_level_limits_rules() {
    let l1 = lint(&fixture(), &LintConfig::level(Level::L1)).unwrap();
    assert!(l1.diagnostics.iter().all(|d| d.level <= Level::L1));
    assert!(l1.diagnostics.iter().any(|d| d.rule == "missing-index"));
    let l2 = lint(&fixture(), &LintConfig::level(Level::L2)).unwrap();
    assert!(!l2.diagnostics.iter().any(|d| d.rule == "broken-link"));
    let disabled = LintConfig {
        disabled: vec!["stale-index".into()],
        ..LintConfig::level(Level::L2)
    };
    assert!(
        !lint(&fixture(), &disabled)
            .unwrap()
            .diagnostics
            .iter()
            .any(|d| d.rule == "stale-index")
    );
    let long = LintConfig {
        max_doc_tokens: 5,
        ..LintConfig::level(Level::L0)
    };
    let r = lint(&fixture(), &long).unwrap();
    assert!(
        r.diagnostics
            .iter()
            .any(|d| d.rule == "doc-too-long" && d.severity == Severity::Warning)
    );
}

#[test]
fn outputs() {
    let report = lint(&fixture(), &LintConfig::level(Level::L3)).unwrap();
    insta::assert_snapshot!("lint_fixture_text", report.to_text());
    let sarif = report.to_sarif();
    assert_eq!(sarif["version"], "2.1.0");
    let results = sarif["runs"][0]["results"].as_array().unwrap();
    assert_eq!(results.len(), report.diagnostics.len());
    let rules = sarif["runs"][0]["tool"]["driver"]["rules"]
        .as_array()
        .unwrap();
    for r in results {
        let idx = r["ruleIndex"].as_u64().unwrap() as usize;
        assert_eq!(rules[idx]["id"], r["ruleId"]);
        assert!(
            r["locations"][0]["physicalLocation"]["region"]["startLine"]
                .as_u64()
                .unwrap()
                >= 1
        );
    }
    let json = serde_json::to_value(&report).unwrap();
    assert_eq!(json["errors"].as_u64().unwrap() as usize, report.errors);
}

fn copy_dir(from: &Path, to: &Path) {
    fs::create_dir_all(to).unwrap();
    for e in fs::read_dir(from).unwrap() {
        let e = e.unwrap();
        if e.file_type().unwrap().is_dir() {
            copy_dir(&e.path(), &to.join(e.file_name()));
        } else {
            fs::copy(e.path(), to.join(e.file_name())).unwrap();
        }
    }
}

#[test]
fn fix_safe_creates_indexes_and_canonicalizes_tags_only() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();
    copy_dir(&fixture(), root);
    let before: Vec<(PathBuf, Vec<u8>)> = okfkit_core::discover(root)
        .unwrap()
        .into_iter()
        .map(|p| {
            let b = fs::read(root.join(&p)).unwrap();
            (p, b)
        })
        .collect();

    let fixed = fix_safe(root, &LintConfig::default()).unwrap();
    assert_eq!(fixed.created, ["l1/nested/index.md"]);
    assert_eq!(fixed.updated, ["l2/synonym-tag.md"]);
    let index = fs::read_to_string(root.join("l1/nested/index.md")).unwrap();
    assert!(
        index.starts_with("# Documents\n\n* [deep](deep.md) - "),
        "{index}"
    );
    let tagged = fs::read_to_string(root.join("l2/synonym-tag.md")).unwrap();
    assert!(tagged.contains("tags: [refund, shipping]\n"));

    // Everything else is byte-identical.
    for (p, bytes) in before {
        if p != Path::new("l2/synonym-tag.md") {
            assert_eq!(fs::read(root.join(&p)).unwrap(), bytes, "{}", p.display());
        }
    }
    let after = lint(root, &LintConfig::level(Level::L3)).unwrap();
    assert!(
        !after
            .diagnostics
            .iter()
            .any(|d| d.rule == "missing-index" || d.rule == "non-canonical-tag")
    );
    // Running it again changes nothing.
    assert_eq!(
        fix_safe(root, &LintConfig::default()).unwrap(),
        Default::default()
    );
}

#[test]
fn unreviewed_generated_fields_are_reported_until_verified() {
    let tmp = tempfile::tempdir().unwrap();
    let doc = "---\ntype: Guide\ntitle: Setup\ndescription: Install the tool and run the setup wizard once.\ngenerated: {by: okfkit-adopt/0.1, at: \"2026-10-01\", fields: [type, description]}\n---\nBody.\n";
    fs::write(
        tmp.path().join("index.md"),
        "# Documents\n\n* [Setup](setup.md)\n",
    )
    .unwrap();
    fs::write(tmp.path().join("setup.md"), doc).unwrap();
    let r = lint(tmp.path(), &LintConfig::level(Level::L1)).unwrap();
    let d: Vec<_> = r
        .diagnostics
        .iter()
        .filter(|d| d.rule == "unreviewed-generated")
        .collect();
    assert_eq!(d.len(), 1);
    assert_eq!((d[0].severity, d[0].line), (Severity::Warning, Some(2)));
    assert!(
        d[0].message
            .starts_with("type, description filled in by a tool")
    );
    fs::write(
        tmp.path().join("setup.md"),
        doc.replace(
            "---\nBody",
            "verified: {by: \"human:an\", at: \"2026-10-02\"}\n---\nBody",
        ),
    )
    .unwrap();
    let r = lint(tmp.path(), &LintConfig::level(Level::L1)).unwrap();
    assert!(
        !r.diagnostics
            .iter()
            .any(|d| d.rule == "unreviewed-generated")
    );
}
