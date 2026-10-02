//! Document conversion for okfkit (docs/PLAN-import.md §1): a thin, controlled wrapper over
//! [anydoc](https://github.com/firecrawl/anydoc) (Word, PowerPoint, OpenDocument, RTF, EPUB),
//! [pdf-inspector](https://github.com/firecrawl/pdf-inspector) (PDF, page by page) and
//! [htmd](https://crates.io/crates/htmd) (HTML).
//!
//! No models, no services and **never hosted OCR**: scanned PDF pages and images are reported
//! (and left for an agent to transcribe) instead of being guessed or sent anywhere. One bad page
//! never loses the rest of a document. Spreadsheets are not converted here: okfkit's data module
//! serves them with SQL.

#![forbid(unsafe_code)]

mod html;

use std::path::Path;

use serde::Serialize;

/// Files larger than this are skipped (reported, never converted).
pub const MAX_BYTES: u64 = 64 * 1024 * 1024;

/// Extensions converted to markdown (lowercase, no dot). Spreadsheets are left to the data module.
pub const EXTENSIONS: &[&str] = &[
    "pdf", "docx", "docm", "doc", "pptx", "pptm", "ppsx", "ppsm", "ppt", "pps", "pot", "odt",
    "odp", "rtf", "epub", "html", "htm", "txt", "rst", "adoc", "png", "jpg", "jpeg",
];

/// Image extensions: always pages to transcribe.
const IMAGES: &[&str] = &["png", "jpg", "jpeg"];

/// Errors.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum Error {
    /// Not a format this crate converts.
    #[error("unsupported format `{0}`")]
    Unsupported(String),
    /// The document is encrypted or password-protected.
    #[error("encrypted or password-protected")]
    Encrypted,
    /// Larger than [`MAX_BYTES`].
    #[error("too large ({0} bytes; limit {MAX_BYTES})")]
    TooLarge(u64),
    /// The converter could not read the document.
    #[error("{0}")]
    Failed(String),
}

/// What needs a human or agent look.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct Flags {
    /// 1-based pages (or `[1]` for an image) with no usable text: scanned or image-only.
    pub needs_ocr: Vec<u32>,
    /// 1-based pages laid out in several columns (reading order may be wrong).
    pub columns: Vec<u32>,
    /// The PDF's fonts have broken encodings (text may be garbled).
    pub encoding_issues: bool,
}

impl Flags {
    /// Nothing to review.
    pub fn is_clean(&self) -> bool {
        self.needs_ocr.is_empty() && self.columns.is_empty() && !self.encoding_issues
    }
}

/// A converted document.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Converted {
    /// Markdown. PDF pages are separated by `<!-- page N -->` markers; pages that need OCR hold a
    /// `> [!NOTE] page N needs OCR` block (or the transcription given in `ocr`).
    pub markdown: String,
    /// The document title (metadata, first heading, or file name).
    pub title: String,
    /// Format name (`pdf`, `docx`, `html`…).
    pub format: String,
    /// Pages (PDF), else `None`.
    pub pages: Option<u32>,
    /// What needs review.
    pub flags: Flags,
    /// Converter and version, for provenance.
    pub converter: &'static str,
}

/// Transcriptions of pages that needed OCR: (1-based page, text).
pub type Ocr<'a> = &'a [(u32, String)];

/// The format of a path, if this crate converts it.
pub fn format_of(path: &Path) -> Option<String> {
    let ext = path.extension()?.to_str()?.to_ascii_lowercase();
    EXTENSIONS.contains(&ext.as_str()).then_some(ext)
}

/// Converts the document at `path` whose content is `bytes`. `ocr` supplies transcriptions of
/// pages that have no text (from an agent); they replace the placeholders.
pub fn convert(path: &Path, bytes: &[u8], ocr: Ocr<'_>) -> Result<Converted, Error> {
    let format = format_of(path).ok_or_else(|| {
        Error::Unsupported(
            path.extension()
                .and_then(|e| e.to_str())
                .unwrap_or("")
                .to_owned(),
        )
    })?;
    if bytes.len() as u64 > MAX_BYTES {
        return Err(Error::TooLarge(bytes.len() as u64));
    }
    let stem = path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("document")
        .replace(['-', '_'], " ");
    let (markdown, meta_title, pages, flags, converter) = match format.as_str() {
        "pdf" => pdf(bytes, ocr)?,
        "html" | "htm" => {
            let (md, title) = html::to_markdown(&String::from_utf8_lossy(bytes))?;
            (md, title, None, Flags::default(), "htmd/0.5")
        }
        "txt" | "rst" | "adoc" => (
            String::from_utf8_lossy(bytes).into_owned(),
            None,
            None,
            Flags::default(),
            "text",
        ),
        f if IMAGES.contains(&f) => {
            let (md, flags) = match ocr.iter().find(|(p, _)| *p == 1) {
                Some((_, text)) => (format!("{}\n", text.trim_end()), Flags::default()),
                None => (
                    "> [!NOTE] page 1 needs OCR: an image without text; transcribe it with `okfkit import ocr-next`.\n"
                        .to_owned(),
                    Flags {
                        needs_ocr: vec![1],
                        ..Default::default()
                    },
                ),
            };
            (md, None, Some(1), flags, "image")
        }
        _ => {
            let md = anydoc::to_markdown_bytes(bytes, anydoc::Format::from_extension(&format))
                .map_err(|e| match e {
                    anydoc::ConvertError::Encrypted => Error::Encrypted,
                    other => Error::Failed(other.to_string()),
                })?;
            (md, None, None, Flags::default(), "anydoc/0.2")
        }
    };
    let title = meta_title
        .filter(|t| !t.trim().is_empty())
        .or_else(|| first_heading(&markdown))
        .unwrap_or(stem);
    Ok(Converted {
        markdown,
        title,
        format,
        pages,
        flags,
        converter,
    })
}

type Parts = (String, Option<String>, Option<u32>, Flags, &'static str);

fn pdf(bytes: &[u8], ocr: Ocr<'_>) -> Result<Parts, Error> {
    let map = |e: pdf_inspector::PdfError| match e {
        pdf_inspector::PdfError::Encrypted => Error::Encrypted,
        other => Error::Failed(format!("{other:?}")),
    };
    let info = pdf_inspector::detect_pdf_mem(bytes).map_err(map)?;
    let pages = pdf_inspector::extract_pages_markdown_mem(bytes, None).map_err(map)?;
    let mut flags = Flags {
        columns: pages.pages_with_columns.clone(),
        encoding_issues: false,
        needs_ocr: Vec::new(),
    };
    let mut out = String::new();
    let count = pages.pages.len();
    for p in &pages.pages {
        let n = p.page + 1;
        if count > 1 {
            out.push_str(&format!("<!-- page {n} -->\n\n"));
        }
        let text = p.markdown.trim();
        if p.needs_ocr || text.is_empty() {
            match ocr.iter().find(|(page, _)| *page == n) {
                Some((_, t)) => out.push_str(t.trim_end()),
                None => {
                    flags.needs_ocr.push(n);
                    if !text.is_empty() {
                        // Text exists but is unreliable (broken encodings): keep it, flagged.
                        flags.encoding_issues = true;
                        out.push_str(text);
                        out.push_str("\n\n");
                    }
                    out.push_str(&format!(
                        "> [!NOTE] page {n} needs OCR: scanned or image-only; transcribe it with `okfkit import ocr-next`."
                    ));
                }
            }
        } else {
            out.push_str(text);
        }
        out.push_str("\n\n");
    }
    let title = info.title.filter(|t| t.trim().len() > 2);
    Ok((
        out.trim_end().to_owned() + "\n",
        title,
        Some(info.page_count),
        flags,
        "pdf-inspector/1.14",
    ))
}

/// The first markdown heading's text.
pub fn first_heading(md: &str) -> Option<String> {
    md.lines()
        .find(|l| l.starts_with('#'))
        .map(|l| l.trim_start_matches('#').trim().to_owned())
        .filter(|t| !t.is_empty())
}

#[cfg(test)]
mod tests;
