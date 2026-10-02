use std::path::Path;

use super::*;

#[test]
fn html_keeps_content_and_rebuilds_tables() {
    let html = r#"<html><head><title>Chính sách</title><script>x()</script></head><body>
<div id="header"><nav><a href="/">Home</a></nav></div>
<div id="main-content"><h1>Đổi trả</h1><p>Trong <b>30 ngày</b>.</p>
<table><tr><td>Loại</td><td>Phí</td></tr><tr><td>Tủ lạnh</td><td>200.000 | đồng</td></tr></table></div>
<footer>Powered by Confluence</footer></body></html>"#;
    let c = convert(Path::new("p.html"), html.as_bytes(), &[]).unwrap();
    assert_eq!(c.title, "Chính sách");
    assert!(
        c.markdown.contains("# Đổi trả") && c.markdown.contains("**30 ngày**"),
        "{}",
        c.markdown
    );
    assert!(
        c.markdown
            .contains("| Loại | Phí |\n| --- | --- |\n| Tủ lạnh | 200.000 \\| đồng |"),
        "{}",
        c.markdown
    );
    assert!(
        !c.markdown.contains("Home")
            && !c.markdown.contains("Powered")
            && !c.markdown.contains("x()")
    );
}

#[test]
fn images_and_text_and_unsupported() {
    let img = convert(Path::new("scan.png"), b"\x89PNG", &[]).unwrap();
    assert_eq!(img.flags.needs_ocr, [1]);
    let done = convert(
        Path::new("scan.png"),
        b"\x89PNG",
        &[(1, "Biên bản họp".into())],
    )
    .unwrap();
    assert!(done.flags.is_clean() && done.markdown.starts_with("Biên bản họp"));
    let t = convert(
        Path::new("notes/read-me.txt"),
        "Ghi chú\nnội dung".as_bytes(),
        &[],
    )
    .unwrap();
    assert_eq!(t.title, "read me");
    assert!(matches!(
        convert(Path::new("a.xlsx"), b"x", &[]),
        Err(Error::Unsupported(_))
    ));
}

/// The S10-lite samples, when generated (`spikes/import-bench/make_samples.py`).
#[test]
fn spike_samples_when_present() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../spikes/import-bench/samples");
    if !dir.is_dir() {
        return;
    }
    let read = |f: &str| std::fs::read(dir.join(f)).unwrap();
    let docx = convert(Path::new("policy-vi.docx"), &read("policy-vi.docx"), &[]).unwrap();
    assert!(
        docx.markdown
            .contains("| Tủ lạnh | 30 ngày | 200.000 đồng |"),
        "{}",
        docx.markdown
    );
    assert_eq!(docx.title, "Chính sách đổi trả hàng");
    let pdf = convert(Path::new("policy-ja.pdf"), &read("policy-ja.pdf"), &[]).unwrap();
    assert!(
        pdf.markdown.contains("返品ポリシー") && pdf.flags.is_clean(),
        "{pdf:?}"
    );
    // A scanned page is reported, not lost or guessed; a transcription fills it in.
    let scan = convert(Path::new("scanned-vi.pdf"), &read("scanned-vi.pdf"), &[]).unwrap();
    assert_eq!(scan.flags.needs_ocr, [1]);
    let fixed = convert(
        Path::new("scanned-vi.pdf"),
        &read("scanned-vi.pdf"),
        &[(1, "# Chính sách đổi trả hàng".into())],
    )
    .unwrap();
    assert!(fixed.flags.needs_ocr.is_empty() && fixed.title == "Chính sách đổi trả hàng");
    let paper = convert(Path::new("arxiv.pdf"), &read("arxiv-attention.pdf"), &[]).unwrap();
    assert_eq!(paper.pages, Some(15));
    assert!(paper.markdown.contains("<!-- page 3 -->"));
}

#[test]
fn converter_names_match_the_locked_versions() {
    let lock = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/../../Cargo.lock"))
        .expect("workspace Cargo.lock");
    for name in [super::ANYDOC, super::HTMD, super::PDF_INSPECTOR] {
        let (krate, minor) = name.split_once('/').unwrap();
        let entry = format!("name = \"{krate}\"\nversion = \"{minor}.");
        assert!(
            lock.contains(&entry),
            "{name} is not the version in Cargo.lock"
        );
    }
}
