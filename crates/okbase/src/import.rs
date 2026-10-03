//! `okbase import`: turn source documents (PDF, Word, PowerPoint, HTML…) into markdown with
//! provenance, safely and repeatably (docs/PLAN-import.md §3–4).
//!
//! Without `--write` nothing changes: sources are already readable directly. Writing produces
//! `<out>/<source path>.md` (split into parts when long), records the source hash and a hash of the
//! written body, and on later runs updates only files whose source changed and whose body nobody
//! edited. Page transcriptions from agents (`ocr_submit`) live in the state dir.

use std::path::{Path, PathBuf};

use serde::Serialize;
use serde_json::json;

use crate::{Bundle, Error, Scope};

/// Documents longer than this many estimated tokens are split into parts at H1/H2 headings.
pub const SPLIT_TOKENS: usize = 6_000;
/// Default folder for imported markdown.
pub const DEFAULT_OUT: &str = "sources";

/// One source document and what importing it does.
#[derive(Debug, Clone, Serialize)]
pub struct Item {
    /// Path of the original.
    pub source: String,
    /// Format.
    pub format: String,
    /// Pages (PDF).
    pub pages: Option<u32>,
    /// 1-based pages without text (need a transcription).
    pub needs_ocr: Vec<u32>,
    /// Multi-column pages (check the reading order).
    pub columns: Vec<u32>,
    /// Markdown files it becomes.
    pub targets: Vec<String>,
    /// `new`, `update` (source changed), `unchanged`, `edited` (body changed by hand; needs
    /// --force), `error`.
    pub action: String,
    /// Why, for errors and conflicts.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
}

/// A page to transcribe.
#[derive(Debug, Clone, Serialize)]
pub struct OcrTask {
    /// Source file (bundle-relative).
    pub path: String,
    /// 1-based page.
    pub page: u32,
    /// Pages in the file, when known.
    pub pages: Option<u32>,
    /// An image of the page to read (exported from the PDF, or the image file itself); `None`
    /// when the page cannot be exported (Group 3 fax or JBIG2 scans): open the PDF page instead.
    pub image: Option<PathBuf>,
}

/// The import plan (or what was done).
#[derive(Debug, Clone, Serialize)]
pub struct Report {
    /// Output folder (bundle-relative).
    pub out: String,
    /// Items, by source path.
    pub items: Vec<Item>,
    /// Whether files were written.
    pub written: bool,
    /// Imported markdown whose source is gone.
    pub orphans: Vec<String>,
    /// Commands to run next.
    pub next: Vec<String>,
}

impl Report {
    /// Text form.
    pub fn to_text(&self) -> String {
        let mut out = String::new();
        for i in &self.items {
            out.push_str(&format!(
                "{:<9} {}{} → {}{}{}\n",
                i.action,
                i.source,
                i.pages.map_or(String::new(), |p| format!(
                    " ({p} page{})",
                    if p == 1 { "" } else { "s" }
                )),
                i.targets.join(", "),
                if i.needs_ocr.is_empty() {
                    String::new()
                } else {
                    format!("; pages needing OCR: {:?}", i.needs_ocr)
                },
                i.detail
                    .as_ref()
                    .map_or(String::new(), |d| format!(" ({d})"))
            ));
        }
        for o in &self.orphans {
            out.push_str(&format!("orphan    {o} (its source no longer exists)\n"));
        }
        if self.items.is_empty() {
            out.push_str("no source documents (PDF, Word, PowerPoint, OpenDocument, RTF, EPUB, HTML, text, images)\n");
        }
        if !self.written
            && self
                .items
                .iter()
                .any(|i| i.action == "new" || i.action == "update")
        {
            out.push_str(&format!(
                "plan only; nothing was written. The sources are already searchable as they are; \
                 `okbase import --write` puts markdown in {}/ for editing.\n",
                self.out
            ));
        }
        for n in &self.next {
            out.push_str(&format!("next: {n}\n"));
        }
        out
    }
}

fn hash(bytes: &[u8]) -> String {
    blake3::hash(bytes).to_hex().to_string()
}

/// Splits converted markdown at H1/H2 headings into parts of at most `SPLIT_TOKENS` (a heading
/// section larger than that stays whole).
fn split(markdown: &str) -> Vec<String> {
    if okbase_analyze::estimate_tokens(markdown) <= SPLIT_TOKENS {
        return vec![markdown.to_owned()];
    }
    let mut sections: Vec<String> = Vec::new();
    for line in markdown.split_inclusive('\n') {
        let heading = line.starts_with("# ") || line.starts_with("## ");
        if heading || sections.is_empty() {
            sections.push(String::new());
        }
        sections.last_mut().expect("pushed").push_str(line);
    }
    let mut parts: Vec<String> = vec![String::new()];
    for s in sections {
        let cur = parts.last_mut().expect("non-empty");
        if !cur.is_empty()
            && okbase_analyze::estimate_tokens(cur) + okbase_analyze::estimate_tokens(&s)
                > SPLIT_TOKENS
        {
            parts.push(s);
        } else {
            cur.push_str(&s);
        }
    }
    parts.retain(|p| !p.trim().is_empty());
    parts
}

fn targets(out: &str, source: &str, parts: usize) -> Vec<String> {
    if parts <= 1 {
        vec![format!("{out}/{source}.md")]
    } else {
        (1..=parts)
            .map(|i| format!("{out}/{source}.part-{i:02}.md"))
            .collect()
    }
}

/// The frontmatter value of an imported file, if it is one: (source path, source hash, body hash).
fn imported_meta(path: &Path) -> Option<(String, String, String, String)> {
    let text = std::fs::read_to_string(path).ok()?;
    let (fm, body) = okbase_core::Frontmatter::split(&text);
    let src = fm.get("source")?;
    Some((
        src.get("path")?.as_str()?.to_owned(),
        src.get("hash")?.as_str()?.to_owned(),
        src.get("body_hash")?.as_str()?.to_owned(),
        hash(body.as_bytes()),
    ))
}

impl Bundle {
    /// Plans (or, with `write`, performs) the import of every source document into `out`.
    /// Writing never overwrites markdown edited by hand unless `force`.
    pub fn import(
        &self,
        out: &str,
        write: bool,
        force: bool,
        scope: &Scope,
    ) -> Result<Report, Error> {
        let root = self.root().to_owned();
        let out = out.trim_matches('/').to_owned();
        let state = self.state_path().ok();
        let files = okbase_core::walk(&root, okbase_index::source_extensions())
            .map_err(|e| Error::Io(e.to_string()))?;
        let mut items = Vec::new();
        let mut produced: Vec<String> = Vec::new();
        for rel in files {
            let source = rel.to_string_lossy().replace('\\', "/");
            if source.starts_with(&format!("{out}/")) || !scope.permits_path(&source) {
                continue;
            }
            let bytes =
                std::fs::read(root.join(&rel)).map_err(|e| Error::Io(format!("{source}: {e}")))?;
            let src_hash = hash(&bytes);
            let ocr = okbase_index::load_ocr(state.as_deref(), &src_hash);
            let converted = match okbase_convert::convert(&rel, &bytes, &ocr) {
                Ok(c) => c,
                Err(e) => {
                    items.push(Item {
                        source,
                        format: rel
                            .extension()
                            .and_then(|e| e.to_str())
                            .unwrap_or("")
                            .to_owned(),
                        pages: None,
                        needs_ocr: vec![],
                        columns: vec![],
                        targets: vec![],
                        action: "error".into(),
                        detail: Some(e.to_string()),
                    });
                    continue;
                }
            };
            let parts = split(&converted.markdown);
            let paths = targets(&out, &source, parts.len());
            produced.extend(paths.iter().cloned());
            // What is on disk now decides the action.
            let mut action = "new";
            let mut detail = None;
            for p in &paths {
                if let Some((_, old_src, body_hash, now)) = imported_meta(&root.join(p)) {
                    if body_hash != now {
                        action = "edited";
                        detail = Some(format!(
                            "{p} was edited after import; --force overwrites it"
                        ));
                        break;
                    }
                    action = if old_src == src_hash {
                        "unchanged"
                    } else {
                        "update"
                    };
                } else if root.join(p).exists() {
                    action = "edited";
                    detail = Some(format!(
                        "{p} exists and was not written by import; --force overwrites it"
                    ));
                    break;
                }
            }
            let do_write =
                write && (action == "new" || action == "update" || (action == "edited" && force));
            if do_write {
                let n = parts.len();
                for (i, (part, path)) in parts.iter().zip(&paths).enumerate() {
                    let title = if n > 1 {
                        format!("{} ({}/{n})", converted.title, i + 1)
                    } else {
                        converted.title.clone()
                    };
                    let mut src = json!({
                        "path": source, "hash": src_hash, "format": converted.format,
                        "converter": converted.converter, "body_hash": hash(part.as_bytes()),
                    });
                    if let Some(p) = converted.pages {
                        src["pages"] = json!(p);
                    }
                    if n > 1 {
                        src["part"] = json!(i + 1);
                        src["parts"] = json!(n);
                    }
                    if !converted.flags.needs_ocr.is_empty() {
                        src["needs_ocr"] = json!(converted.flags.needs_ocr);
                    }
                    let fm = json!({
                        "type": "Source",
                        "title": title,
                        "source": src,
                        "generated": {"by": "okbase-import", "at": today(), "fields": ["type", "title"]},
                    });
                    let text = format!("---\n{fm}\n---\n{part}");
                    let full = root.join(path);
                    if let Some(parent) = full.parent() {
                        std::fs::create_dir_all(parent).map_err(|e| Error::Io(e.to_string()))?;
                    }
                    std::fs::write(&full, text).map_err(|e| Error::Io(format!("{path}: {e}")))?;
                }
            }
            items.push(Item {
                source,
                format: converted.format,
                pages: converted.pages,
                needs_ocr: converted.flags.needs_ocr,
                columns: converted.flags.columns,
                targets: paths,
                action: action.into(),
                detail,
            });
        }
        // Imported files whose source is gone.
        let mut orphans = Vec::new();
        if let Ok(md) = okbase_core::walk(&root.join(&out), &["md"]) {
            for rel in md {
                let p = format!("{out}/{}", rel.to_string_lossy().replace('\\', "/"));
                if let Some((src, ..)) = imported_meta(&root.join(&p))
                    && !root.join(&src).exists()
                {
                    orphans.push(p);
                }
            }
        }
        let mut next = Vec::new();
        if items.iter().any(|i| !i.needs_ocr.is_empty()) {
            next.push("okbase import ocr-next   (an agent transcribes pages without text)".into());
        }
        if !write
            && items
                .iter()
                .any(|i| i.action == "new" || i.action == "update")
        {
            next.push(format!(
                "okbase import --write   (markdown in {out}/ for editing; ask the user first)"
            ));
        }
        if write {
            next.push("okbase lint --level L1".into());
        }
        Ok(Report {
            out,
            items,
            written: write,
            orphans,
            next,
        })
    }

    /// The next source page that needs a transcription. For a scanned PDF page the page image is
    /// written next to the transcriptions (state dir) so any agent that reads images can work
    /// from it; for an image file, the file itself.
    pub fn ocr_next(&self, scope: &Scope) -> Result<Option<OcrTask>, Error> {
        // Collect first: the index lock must not be held across the loop (state_path locks it).
        let pending = self.index().sources_needing_ocr()?;
        for p in pending {
            if !scope.permits_path(&p.path) {
                continue;
            }
            let Some(&page) = p.pages.first() else {
                continue;
            };
            let full = self.root().join(&p.path);
            let ext = full
                .extension()
                .and_then(|e| e.to_str())
                .unwrap_or("")
                .to_ascii_lowercase();
            let image = if matches!(ext.as_str(), "png" | "jpg" | "jpeg") {
                Some(full.canonicalize().unwrap_or(full))
            } else if ext == "pdf" {
                let bytes =
                    std::fs::read(&full).map_err(|e| Error::Io(format!("{}: {e}", p.path)))?;
                match (okbase_convert::page_image(&bytes, page), self.state_path()) {
                    (Some(img), Ok(state)) => {
                        let dir = okbase_index::ocr_dir(&state, &hash(&bytes));
                        std::fs::create_dir_all(&dir).map_err(|e| Error::Io(e.to_string()))?;
                        let file = dir.join(format!("page-{page}.{}", img.ext));
                        std::fs::write(&file, &img.bytes).map_err(|e| Error::Io(e.to_string()))?;
                        Some(file)
                    }
                    _ => None,
                }
            } else {
                None
            };
            return Ok(Some(OcrTask {
                path: p.path.clone(),
                page,
                pages: p.page_count,
                image,
            }));
        }
        Ok(None)
    }

    /// Stores a transcription of `page` of the source `path` (state dir; the bundle is not
    /// touched) and re-reads that source.
    pub fn ocr_submit(&self, path: &str, page: u32, text: &str) -> Result<PathBuf, Error> {
        let state = self.state_path()?;
        let bytes =
            std::fs::read(self.root().join(path)).map_err(|e| Error::Io(format!("{path}: {e}")))?;
        let dir = okbase_index::ocr_dir(&state, &hash(&bytes));
        std::fs::create_dir_all(&dir).map_err(|e| Error::Io(e.to_string()))?;
        let file = dir.join(format!("{page}.md"));
        std::fs::write(&file, text).map_err(|e| Error::Io(e.to_string()))?;
        self.index().invalidate(path)?;
        self.sync()?;
        Ok(file)
    }
}

/// Today's date (UTC) as `YYYY-MM-DD`, without a date-time dependency.
pub fn today() -> String {
    let days = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs() / 86_400) as i64;
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    format!("{:04}-{m:02}-{d:02}", yoe + era * 400 + i64::from(m <= 2))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn long_documents_split_at_headings() {
        let section = format!("## Part\n\n{}\n\n", "word ".repeat(2_500));
        let md = format!("# Manual\n\nIntro.\n\n{}", section.repeat(4));
        let parts = split(&md);
        assert!(parts.len() >= 2, "{}", parts.len());
        assert!(parts.iter().skip(1).all(|p| p.starts_with("## Part")));
        assert_eq!(parts.concat(), md);
        assert_eq!(split("# Short\n\nText.\n").len(), 1);
        assert_eq!(targets("sources", "a/b.pdf", 1), ["sources/a/b.pdf.md"]);
        assert_eq!(
            targets("sources", "a/b.pdf", 2)[1],
            "sources/a/b.pdf.part-02.md"
        );
    }
}
