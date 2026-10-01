//! Adopting the OpenClaw fixture and small synthetic folders.

use std::fs;
use std::path::{Path, PathBuf};

use okfkit_adopt::{AdoptOptions, ChangeKind, Site, apply_in_place, apply_to, plan};
use okfkit_lint::{Level, LintConfig, lint};

fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../fixtures")
        .join(name)
}

fn opts() -> AdoptOptions {
    AdoptOptions::new("2026-10-01")
}

#[test]
fn openclaw_reaches_l1_without_manual_edits() {
    let src = fixture("openclaw-s");
    let p = plan(&src, &opts()).unwrap();
    assert_eq!(p.level_before, None);
    assert_eq!(p.level_after, Some(Level::L1), "{}", p.to_text(true));
    insta::assert_snapshot!("openclaw_plan", p.to_text(false));

    let tmp = tempfile::tempdir().unwrap();
    let out = tmp.path().join("okf");
    apply_to(&src, &p, &out).unwrap();
    let report = lint(&out, &LintConfig::level(Level::L1)).unwrap();
    assert_eq!(report.errors, 0, "{}", report.to_text());
    assert_eq!(report.level, Some(Level::L1));

    // Existing frontmatter lines are kept verbatim; new keys are appended.
    for c in &p.changes {
        if c.kind == ChangeKind::Modify && c.path.ends_with(".md") && c.path != "log.md" {
            let before = fs::read_to_string(src.join(&c.path)).unwrap();
            let after = fs::read_to_string(out.join(&c.path)).unwrap();
            let fm_before = before.split("\n---\n").next().unwrap();
            assert!(after.starts_with(fm_before), "{}", c.path);
            assert!(
                after.ends_with(before.split_once("\n---\n").unwrap().1),
                "body changed in {}",
                c.path
            );
        }
    }
    // Adopting the result again changes nothing but the log.
    let again = plan(&out, &opts()).unwrap();
    assert!(again.changes.is_empty(), "{}", again.to_text(true));
    // The source was not touched.
    assert!(
        !src.join("index.md").exists()
            || fs::read_to_string(src.join("index.md"))
                .unwrap()
                .contains("OpenClaw")
    );
}

fn write(root: &Path, rel: &str, text: &str) {
    let p = root.join(rel);
    fs::create_dir_all(p.parent().unwrap()).unwrap();
    fs::write(p, text).unwrap();
}

#[test]
fn plain_folder_in_place_with_l2() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();
    write(root, ".obsidian/app.json", "{}");
    write(
        root,
        "index.md",
        "# Team wiki\n\nWelcome to the wiki of the support team, where we keep our runbooks.\n",
    );
    write(
        root,
        "runbooks/restart.md",
        "# Restart the gateway\n\n:::warning\nCareful.\n:::\n\nRestart the gateway with `systemctl restart gw` when it stops answering.\n",
    );
    write(
        root,
        "faq/refunds.md",
        "---\ntags: [billing]\n# keep this comment\n---\n# Hoàn tiền\n\nChính sách hoàn tiền áp dụng trong vòng 30 ngày kể từ ngày mua hàng.\n",
    );
    write(root, "notes/empty.md", "```\nonly code\n```\n");

    let mut o = opts();
    o.level = Level::L2;
    let p = plan(root, &o).unwrap();
    assert_eq!(p.site, Site::Obsidian);
    assert!(p.changes.iter().any(|c| c.path == "overview.md"
        && matches!(&c.kind, ChangeKind::Rename { from } if from == "index.md")));
    apply_in_place(root, &p).unwrap();

    let restart = fs::read_to_string(root.join("runbooks/restart.md")).unwrap();
    assert!(restart.contains("type: Playbook\n"), "{restart}");
    assert!(restart.contains("title: Restart the gateway\n"));
    assert!(restart.contains("description: Restart the gateway with `systemctl restart gw` when it stops answering.\n"), "{restart}");
    assert!(
        restart.contains("lang: en")
            && restart.contains("status: stable")
            && restart.contains("tags: [runbooks]")
    );
    let refunds = fs::read_to_string(root.join("faq/refunds.md")).unwrap();
    assert!(
        refunds.starts_with("---\ntags: [billing]\n# keep this comment\n"),
        "{refunds}"
    );
    assert!(refunds.contains("type: FAQ") && refunds.contains("lang: vi"));
    assert!(!refunds.contains("tags: [faq]"), "existing tags are kept");
    let empty = fs::read_to_string(root.join("notes/empty.md")).unwrap();
    assert!(empty.contains("description: Empty (Document)."), "{empty}");
    assert!(p.weak_descriptions.contains(&"notes/empty.md".to_owned()));
    let index = fs::read_to_string(root.join("index.md")).unwrap();
    assert!(
        index.starts_with("---\nokf_version: \"0.2\"\n---\n# Subdirectories\n"),
        "{index}"
    );
    assert!(index.contains("* [Team wiki](overview.md)"));
    assert!(
        fs::read_to_string(root.join("log.md"))
            .unwrap()
            .contains("## 2026-10-01\n* **Adoption**")
    );
    let report = lint(root, &LintConfig::level(Level::L1)).unwrap();
    assert_eq!(report.errors, 0, "{}", report.to_text());
}
