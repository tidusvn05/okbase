use std::collections::HashSet;
use std::path::Path;

use okbase_index::{Index, IndexOptions, StateDir};

use super::*;

fn fixture(name: &str) -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../fixtures")
        .join(name)
}

fn index(bundle: &Path, state: &Path) -> Index {
    let mut idx = Index::open(
        bundle,
        &IndexOptions {
            state_dir: StateDir::Path(state.to_owned()),
            ..Default::default()
        },
    )
    .unwrap();
    idx.sync().unwrap();
    idx
}

fn settings() -> Settings {
    Settings {
        langs: vec!["vi".into(), "ja".into(), "en".into()],
        ..Default::default()
    }
}

#[test]
fn init_samples_splits_and_batches_deterministically() {
    let st = tempfile::tempdir().unwrap();
    let idx = index(&fixture("multilingual"), &st.path().join("idx"));
    let scope = okbase_query::Scope::all();
    let a = init(&idx, &scope, st.path(), settings()).unwrap();
    let plan = a.plan().clone();
    assert_eq!(plan.standard, STANDARD);
    assert_eq!(plan.docs, 100);
    // 15% of 100 documents, at least min(20, 100/4).
    assert_eq!(plan.heldout_docs.len(), 20);
    let mut seen = HashSet::new();
    let mut whole = 0;
    for n in 1..=plan.batches {
        let b = a.batch(n).unwrap();
        assert!(b.len() <= 10);
        for p in b {
            assert!(seen.insert(p.key.clone()), "{} twice", p.key);
            whole += usize::from(p.whole);
            assert!(["vi", "en", "ja"].contains(&p.lang.as_str()), "{}", p.lang);
        }
    }
    assert_eq!(seen.len(), plan.passages);
    assert!(whole > 50, "short fixture docs are single chunks");
    // Same bundle, same sample.
    let b = init(&idx, &scope, st.path(), settings()).unwrap();
    assert_ne!(a.plan().id, b.plan().id);
    assert_eq!(b.plan().heldout_docs, plan.heldout_docs);
    assert_eq!(b.batch(1).unwrap(), a.batch(1).unwrap());
    assert_eq!(runs(st.path()).len(), 2);
    assert_eq!(Run::find(st.path(), None).unwrap().plan().id, b.plan().id);
    // Scope is applied.
    let only_vi = okbase_query::Scope::all().allow("knowledge/vi-*").unwrap();
    let c = init(&idx, &only_vi, st.path(), settings()).unwrap();
    assert!(
        c.batch(1)
            .unwrap()
            .iter()
            .all(|p| p.doc.starts_with("knowledge/vi-"))
    );
}

#[test]
fn claim_submit_status_finalize() {
    let st = tempfile::tempdir().unwrap();
    let bundle = st.path().join("kb");
    std::fs::create_dir_all(&bundle).unwrap();
    let body = "Employees at the Tokyo and Osaka offices receive ten days of paid annual leave once they \
                have worked for six months, rising by one or two days each year up to a maximum of twenty \
                days. Unused days carry over for one year and then expire.";
    std::fs::write(
        bundle.join("leave.md"),
        format!("---\ntitle: Annual leave\ndescription: Paid leave rules.\n---\n\n{body}\n"),
    )
    .unwrap();
    let idx = index(&bundle, &st.path().join("idx"));
    let s = Settings {
        min_pairs: 1,
        min_heldout_docs: 0,
        heldout_share: 0.0,
        ..settings()
    };
    let run = init(&idx, &okbase_query::Scope::all(), st.path(), s).unwrap();
    assert_eq!(run.plan().batches, 1);
    let view = run.next(true).unwrap().unwrap();
    assert_eq!(view.passages[0].questions, 4);
    assert!(
        view.to_text()
            .contains("passage leave#0 [4] lang: en | Annual leave")
    );
    // Claimed: a second agent gets nothing.
    assert!(run.next(true).unwrap().is_none());
    let bad = run
        .submit(
            1,
            r#"{"passage": "leave#0", "kind": "natural", "lang": "en", "q": "?"}"#,
            &HashSet::new(),
        )
        .unwrap();
    assert!(!bad.accepted && bad.errors.len() >= 2, "{bad:?}");
    let good = r#"{"passage": "leave#0", "kind": "natural", "lang": "en", "q": "How much vacation do new hires in Japan get?"}
{"passage": "leave#0", "kind": "keyword", "lang": "en", "q": "annual leave carry over"}
{"passage": "leave#0", "kind": "cross", "lang": "vi", "q": "Ngày phép chưa dùng có được chuyển sang năm sau không?"}
{"passage": "leave#0", "kind": "vague", "lang": "ja", "q": "休みが余ったらどうなるの"}"#;
    let ok = run.submit(1, good, &HashSet::new()).unwrap();
    assert!(ok.accepted, "{ok:?}");
    assert_eq!(
        (ok.remaining, ok.next.as_str()),
        (0, "okbase embed tune check")
    );
    let status = run.finalize().unwrap();
    assert!(status.ready);
    assert_eq!(status.train_pairs, 4);
    assert_eq!(status.kinds.len(), 4);
    let train = std::fs::read_to_string(run.train_path()).unwrap();
    assert_eq!(train.lines().count(), 4);
    assert!(train.contains("\"title\":\"Annual leave\""), "{train}");
    // Human-written eval questions must not be repeated.
    let seen: HashSet<String> = [normalize("annual leave carry over")].into();
    let again = run.submit(1, good, &seen).unwrap();
    assert!(again.errors.iter().any(|e| e.contains("duplicate")));
}

#[test]
fn guide_names_every_step() {
    for cmd in [
        "tune init",
        "tune next",
        "tune submit",
        "tune check",
        "tune train",
        "tune export",
        "tune eval",
        "tune activate",
        "tune rollback",
    ] {
        assert!(GUIDE.contains(cmd), "{cmd}");
    }
}
