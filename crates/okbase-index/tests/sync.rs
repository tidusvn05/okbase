//! Incremental sync, determinism and speed of the index.

use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use okbase_index::{BUNDLE_STATE_DIR, Index, IndexOptions, StateDir};
use rusqlite::Connection;

fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../fixtures")
        .join(name)
}

fn copy_dir(from: &Path, to: &Path) {
    fs::create_dir_all(to).unwrap();
    for e in fs::read_dir(from).unwrap() {
        let e = e.unwrap();
        let target = to.join(e.file_name());
        if e.file_type().unwrap().is_dir() {
            copy_dir(&e.path(), &target);
        } else {
            fs::copy(e.path(), target).unwrap();
        }
    }
}

/// Every table's content, independent of row ids and file times.
fn dump(conn: &Connection) -> Vec<String> {
    let queries = [
        "SELECT id, path, hash, size, reserved, fm_state, type, title, title_from, description, description_from, \
         lang, status, updated, tokens, frontmatter, fm_raw, body FROM docs ORDER BY id",
        "SELECT * FROM doc_fields ORDER BY doc_id, key, idx",
        "SELECT * FROM doc_tags ORDER BY doc_id, tag",
        "SELECT * FROM aliases ORDER BY doc_id, alias",
        "SELECT * FROM links ORDER BY src, line, raw, kind",
        "SELECT doc_id, ord, heading, text, start_line, end_line, tokens FROM chunks ORDER BY doc_id, ord",
        "SELECT c.doc_id, c.ord, f.terms FROM chunks_fts f JOIN chunks c ON c.id = f.rowid ORDER BY c.doc_id, c.ord",
    ];
    let mut out = Vec::new();
    for q in queries {
        let mut st = conn.prepare(q).unwrap();
        let n = st.column_count();
        let rows = st
            .query_map([], |r| {
                Ok((0..n)
                    .map(|i| format!("{:?}", r.get_ref(i).unwrap()))
                    .collect::<Vec<_>>()
                    .join(" | "))
            })
            .unwrap();
        out.extend(rows.map(Result::unwrap));
    }
    out
}

fn open(root: &Path) -> Index {
    Index::open(root, &IndexOptions::default()).unwrap()
}

#[test]
fn indexes_fixture_with_links_tags_and_chunks() {
    let mut idx = Index::open_in_memory(&fixture("okf-official/acme_retail")).unwrap();
    let stats = idx.sync().unwrap();
    assert_eq!((stats.added, stats.updated, stats.removed), (17, 0, 0));
    assert!(stats.skipped.is_empty());
    let c = idx.connection();
    let count = |q: &str| c.query_row(q, [], |r| r.get::<_, i64>(0)).unwrap();
    assert_eq!(count("SELECT COUNT(*) FROM docs WHERE reserved = 0"), 9);
    assert!(count("SELECT COUNT(*) FROM chunks") >= 9);
    assert!(count("SELECT COUNT(*) FROM links WHERE target IN (SELECT id FROM docs)") > 5);
    assert!(count("SELECT COUNT(*) FROM doc_tags") > 0);
    let hits = count("SELECT COUNT(*) FROM chunks_fts WHERE chunks_fts MATCH 'revenu'");
    assert!(hits > 0, "stemmed FTS match for 'revenue'");
    // A second sync with no changes touches nothing.
    let again = idx.sync().unwrap();
    assert_eq!(
        (again.added, again.updated, again.removed, again.unchanged),
        (0, 0, 0, 17)
    );
}

#[test]
fn incremental_sync_reindexes_only_changed_files() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path().join("bundle");
    copy_dir(&fixture("openclaw-s"), &root);
    let mut idx = open(&root);
    let first = idx.sync().unwrap();
    let total = first.added;
    assert!(total >= 30);
    assert!(root.join(BUNDLE_STATE_DIR).join(".gitignore").is_file());

    // Change one file's content.
    let page = root.join("gateway/audit.md");
    let text = fs::read_to_string(&page).unwrap();
    fs::write(&page, text.replace("OpenClaw", "OpenClaw (edited)")).unwrap();
    let s = idx.sync().unwrap();
    assert_eq!(
        (s.added, s.updated, s.removed, s.unchanged),
        (0, 1, 0, total - 1)
    );

    // Rewrite a file with identical bytes: new mtime, same hash, nothing reindexed.
    let other = root.join("gateway/doctor.md");
    fs::write(&other, fs::read(&other).unwrap()).unwrap();
    let s = idx.sync().unwrap();
    assert_eq!((s.added, s.updated, s.unchanged), (0, 0, total));

    // Add and remove.
    fs::write(
        root.join("gateway/new.md"),
        "---\ntype: Guide\ntitle: New\n---\n# New\n\nSee [[doctor]].\n",
    )
    .unwrap();
    fs::remove_file(root.join("gateway/bonjour.md")).unwrap();
    let s = idx.sync().unwrap();
    assert_eq!((s.added, s.removed), (1, 1));
    let wiki: Option<String> = idx
        .connection()
        .query_row(
            "SELECT target FROM links WHERE src = 'gateway/new' AND kind = 'wiki'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(wiki.as_deref(), Some("gateway/doctor"));

    // The incrementally maintained index equals a fresh build of the same files.
    let incremental = dump(idx.connection());
    drop(idx);
    fs::remove_dir_all(root.join(BUNDLE_STATE_DIR)).unwrap();
    let mut fresh = open(&root);
    fresh.sync().unwrap();
    assert_eq!(dump(fresh.connection()), incremental);
}

#[test]
fn explicit_state_dir_keeps_the_bundle_untouched() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path().join("bundle");
    copy_dir(&fixture("okf-official/ga4"), &root);
    let state = tmp.path().join("state");
    let mut idx = Index::open(
        &root,
        &IndexOptions {
            state_dir: StateDir::Path(state.clone()),
            ..Default::default()
        },
    )
    .unwrap();
    idx.sync().unwrap();
    assert!(state.join("index.sqlite").is_file());
    assert!(!root.join(BUNDLE_STATE_DIR).exists());
}

/// Acceptance: a first index of 1k documents takes at most 5 s.
/// Run with `cargo test --release -p okbase-index -- --ignored`.
#[test]
#[ignore = "timing test; run in release mode"]
fn first_index_of_1k_docs_under_5s() {
    let tmp = tempfile::tempdir().unwrap();
    let src = fixture("openclaw-s");
    let mut n = 0;
    for copy in 0.. {
        copy_dir(&src, &tmp.path().join(format!("copy{copy:02}")));
        n += 31;
        if n >= 1000 {
            break;
        }
    }
    let state = tempfile::tempdir().unwrap();
    let opts = IndexOptions {
        state_dir: StateDir::Path(state.path().to_owned()),
        ..Default::default()
    };
    let start = Instant::now();
    let mut idx = Index::open(tmp.path(), &opts).unwrap();
    let stats = idx.sync().unwrap();
    let took = start.elapsed();
    eprintln!("indexed {} docs in {took:?}", stats.added);
    assert!(stats.added >= 1000);
    assert!(took < Duration::from_secs(5), "took {took:?}");
}
