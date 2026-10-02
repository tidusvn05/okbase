//! Direct reading of source documents (PDF, Word, PowerPoint, HTML…; docs/PLAN-import.md §2):
//! converted while indexing, never written into the bundle. A source whose markdown was
//! materialized (`okfkit import --write`, frontmatter `source.path`) is not read twice.

use std::path::{Path, PathBuf};

use okfkit_core::{Concept, ConceptId, Frontmatter};
use serde_json::json;

/// File extensions read directly (empty without the `import` feature).
pub fn extensions() -> &'static [&'static str] {
    #[cfg(feature = "import")]
    {
        okfkit_convert::EXTENSIONS
    }
    #[cfg(not(feature = "import"))]
    {
        &[]
    }
}

/// Whether a bundle-relative path is a source document (not markdown).
pub fn is_source(rel: &Path) -> bool {
    rel.extension().and_then(|e| e.to_str()).is_some_and(|e| {
        !e.eq_ignore_ascii_case("md") && extensions().contains(&e.to_ascii_lowercase().as_str())
    })
}

/// Where agent transcriptions of pages live: `<state>/ocr/<file hash>/<page>.md`.
pub fn ocr_dir(state: &Path, hash: &str) -> PathBuf {
    state.join("ocr").join(hash)
}

/// Transcriptions recorded for a file hash.
pub fn load_ocr(state: Option<&Path>, hash: &str) -> Vec<(u32, String)> {
    let Some(state) = state else {
        return Vec::new();
    };
    let Ok(entries) = std::fs::read_dir(ocr_dir(state, hash)) else {
        return Vec::new();
    };
    let mut out: Vec<(u32, String)> = entries
        .flatten()
        .filter_map(|e| {
            let page = e.path().file_stem()?.to_str()?.parse().ok()?;
            Some((page, std::fs::read_to_string(e.path()).ok()?))
        })
        .collect();
    out.sort();
    out
}

/// What the index records about a converted source.
#[derive(Debug, Clone, Default)]
pub struct SourceInfo {
    pub format: String,
    pub pages: Option<u32>,
    pub needs_ocr: Vec<u32>,
    pub columns: Vec<u32>,
    pub encoding_issues: bool,
    pub converter: String,
}

/// The first sentence of prose in converted markdown (for the description).
fn first_sentence(md: &str) -> Option<String> {
    let line = md.lines().map(str::trim).find(|l| {
        !l.is_empty()
            && !l.starts_with('#')
            && !l.starts_with('|')
            && !l.starts_with("<!--")
            && !l.starts_with('>')
            && !l.starts_with("```")
            && l.chars().filter(|c| c.is_alphabetic()).count() >= 12
    })?;
    let line = line.trim_start_matches(['-', '*', ' ']).trim();
    let mut out = String::new();
    for (i, c) in line.char_indices() {
        out.push(c);
        if matches!(c, '.' | '。' | '!' | '?') && i > 20 {
            break;
        }
        if out.chars().count() >= 200 {
            out.push('…');
            break;
        }
    }
    Some(out)
}

/// Converts a source file into a concept (synthesized frontmatter + markdown body).
#[cfg(feature = "import")]
pub fn convert(
    rel: &Path,
    id: &ConceptId,
    bytes: &[u8],
    ocr: &[(u32, String)],
) -> Result<(Concept, SourceInfo), String> {
    let c = okfkit_convert::convert(rel, bytes, ocr).map_err(|e| e.to_string())?;
    let path = rel.to_string_lossy().replace('\\', "/");
    let mut source = json!({"path": path, "format": c.format});
    if let Some(p) = c.pages {
        source["pages"] = json!(p);
    }
    if !c.flags.needs_ocr.is_empty() {
        source["needs_ocr"] = json!(c.flags.needs_ocr);
    }
    let mut fm = json!({
        "type": "Source",
        "title": c.title,
        "source": source,
        "generated": {"by": c.converter, "fields": ["type", "title", "description"]},
    });
    if let Some(d) = first_sentence(&c.markdown) {
        fm["description"] = json!(d);
    }
    let text = format!("---\n{}\n---\n{}", fm, c.markdown);
    let (frontmatter, body) = Frontmatter::split(&text);
    Ok((
        Concept {
            id: id.clone(),
            path,
            frontmatter,
            body: body.to_owned(),
        },
        SourceInfo {
            format: c.format,
            pages: c.pages,
            needs_ocr: c.flags.needs_ocr,
            columns: c.flags.columns,
            encoding_issues: c.flags.encoding_issues,
            converter: c.converter.to_owned(),
        },
    ))
}

/// Without the `import` feature nothing is a source.
#[cfg(not(feature = "import"))]
pub fn convert(
    _: &Path,
    _: &ConceptId,
    _: &[u8],
    _: &[(u32, String)],
) -> Result<(Concept, SourceInfo), String> {
    Err("document import is not in this build".into())
}

#[cfg(test)]
mod tests {
    #[test]
    fn first_sentence_skips_markup() {
        let md = "<!-- page 1 -->\n\n# Title\n\n| a | b |\n\nCustomers may return products within 30 days. More text.\n";
        assert_eq!(
            super::first_sentence(md).as_deref(),
            Some("Customers may return products within 30 days.")
        );
    }
}
