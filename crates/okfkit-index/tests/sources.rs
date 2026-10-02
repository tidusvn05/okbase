//! Direct reading of source documents (needs the `import` feature, on by default).
#![cfg(feature = "import")]

use std::path::Path;

use okfkit_index::{Index, IndexOptions, StateDir};

fn samples() -> Option<std::path::PathBuf> {
    let d = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../spikes/import-bench/samples");
    d.is_dir().then_some(d)
}

#[test]
fn sources_are_read_directly_and_give_way_to_imports() {
    let tmp = tempfile::tempdir().unwrap();
    let kb = tmp.path().join("kb");
    std::fs::create_dir_all(kb.join("manuals")).unwrap();
    std::fs::write(
        kb.join("manuals/returns.html"),
        "<html><head><title>Returns</title></head><body><nav>Home</nav><main><h1>Returns</h1>\
         <p>Customers may return products within 30 days of delivery.</p></main></body></html>",
    )
    .unwrap();
    std::fs::write(
        kb.join("notes.txt"),
        "Office hours\nMonday to Friday, 9 to 18.\n",
    )
    .unwrap();
    let have_samples = samples().is_some();
    if let Some(s) = samples() {
        std::fs::copy(s.join("policy-vi.docx"), kb.join("policy-vi.docx")).unwrap();
        std::fs::copy(s.join("scanned-vi.pdf"), kb.join("scanned.pdf")).unwrap();
    }
    let state = tmp.path().join("state");
    let mut idx = Index::open(
        &kb,
        &IndexOptions {
            state_dir: StateDir::Path(state.clone()),
            ..Default::default()
        },
    )
    .unwrap();
    let st = idx.sync().unwrap();
    assert!(st.skipped.is_empty(), "{:?}", st.skipped);
    let conn = idx.connection();
    let one = |sql: &str| -> String { conn.query_row(sql, [], |r| r.get(0)).unwrap() };
    assert_eq!(
        one("SELECT title FROM docs WHERE id = 'manuals/returns.html'"),
        "Returns"
    );
    assert_eq!(
        one("SELECT description FROM docs WHERE id = 'manuals/returns.html'"),
        "Customers may return products within 30 days of delivery."
    );
    assert_eq!(
        one("SELECT type FROM docs WHERE id = 'notes.txt'"),
        "Source"
    );
    assert!(!one("SELECT body FROM docs WHERE id = 'manuals/returns.html'").contains("Home"));
    if have_samples {
        assert!(
            one("SELECT body FROM docs WHERE id = 'policy-vi.docx'")
                .contains("| Tủ lạnh | 30 ngày | 200.000 đồng |")
        );
        assert_eq!(
            one("SELECT status FROM sources WHERE path = 'scanned.pdf'"),
            "partial"
        );
        assert_eq!(
            one("SELECT needs_ocr FROM sources WHERE path = 'scanned.pdf'"),
            "[1]"
        );
        // An agent transcribes page 1: the next sync uses it.
        let hash = one("SELECT hash FROM sources WHERE path = 'scanned.pdf'");
        let dir = okfkit_index::ocr_dir(&state, &hash);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join("1.md"),
            "# Chính sách đổi trả hàng\n\nĐổi trả trong 30 ngày.\n",
        )
        .unwrap();
        idx.invalidate("scanned.pdf").unwrap();
        idx.sync().unwrap();
        let conn = idx.connection();
        let body: String = conn
            .query_row("SELECT body FROM docs WHERE id = 'scanned.pdf'", [], |r| {
                r.get(0)
            })
            .unwrap();
        assert!(body.contains("Đổi trả trong 30 ngày."), "{body}");
        let status: String = conn
            .query_row(
                "SELECT status FROM sources WHERE path = 'scanned.pdf'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(status, "ok");
    }
    // Once its markdown is imported, the source is not read twice.
    std::fs::write(
        kb.join("returns.md"),
        "---\ntype: Policy\ntitle: Returns\ndescription: Returns within 30 days.\nsource:\n  path: manuals/returns.html\n---\n# Returns\n",
    )
    .unwrap();
    idx.sync().unwrap();
    let n: i64 = idx
        .connection()
        .query_row(
            "SELECT COUNT(*) FROM docs WHERE id = 'manuals/returns.html'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(n, 0);
}
