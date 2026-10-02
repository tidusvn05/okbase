//! Starting a knowledge base (`okfkit init`) and adding a document (`okfkit new`).
//!
//! `init` lays out an empty folder as an OKF bundle at level L2 from the first document on:
//! a root `index.md` with `okf_version`, a tag vocabulary, type schemas, a log and `okfkit.toml`.
//! `new` creates one document with the frontmatter its type needs and lists it in its folder's
//! `index.md`. Both only create files; they never overwrite.

use std::fmt::Write as _;
use std::path::{Path, PathBuf};

use crate::Error;

/// Types `init` creates schemas for when none are given.
pub const DEFAULT_TYPES: &[&str] = &["Guide", "Policy", "FAQ"];

/// What `init` writes.
#[derive(Debug, Clone)]
pub struct InitOptions {
    /// What the knowledge base is about (the root index title).
    pub title: String,
    /// One sentence about it.
    pub description: String,
    /// Languages people write and ask in (ISO 639-1).
    pub langs: Vec<String>,
    /// Document types to declare.
    pub types: Vec<String>,
    /// Today (`YYYY-MM-DD`).
    pub today: String,
}

fn io(path: &Path) -> impl FnOnce(std::io::Error) -> Error + '_ {
    move |source| Error::Io {
        path: path.to_owned(),
        source,
    }
}

/// A TOML basic string (JSON string escapes are valid TOML).
fn toml_str(s: &str) -> String {
    serde_json::to_string(s).unwrap_or_else(|_| format!("\"{s}\""))
}

/// A YAML scalar: plain when that is unambiguous, else double-quoted.
fn yaml_str(s: &str) -> String {
    let plain = !s.is_empty()
        && s.trim() == s
        && !s.contains(": ")
        && !s.contains(" #")
        && !s.ends_with(':')
        && !s.starts_with(|c: char| "-?:,[]{}#&*!|>'\"%@`".contains(c) || c.is_ascii_digit())
        && !matches!(
            s.to_lowercase().as_str(),
            "true" | "false" | "yes" | "no" | "null" | "~" | "on" | "off"
        );
    if plain {
        s.to_owned()
    } else {
        serde_json::to_string(s).unwrap_or_else(|_| format!("\"{s}\""))
    }
}

/// Schema fields for well-known types (others start empty).
fn schema_fields(ty: &str) -> &'static str {
    match ty {
        "Policy" => "  owner: {type: string, required: true}\n  effective_from: date\n",
        "Guide" | "Tutorial" => "  audience: string\n",
        "FAQ" => "  product: string\n",
        _ => "",
    }
}

/// The files `init` would create, as (relative path, content). Fails if the folder already holds
/// markdown documents (use `adopt` for those).
pub fn init_files(root: &Path, o: &InitOptions) -> Result<Vec<(String, String)>, Error> {
    let existing = okfkit_core::discover(root)?;
    if !existing.is_empty() {
        return Err(Error::Invalid(format!(
            "{} already has {} markdown documents; use `okfkit adopt` to convert them, or `okfkit onboard`",
            root.display(),
            existing.len()
        )));
    }
    let langs = if o.langs.is_empty() {
        vec!["en".to_owned()]
    } else {
        o.langs.clone()
    };
    let types: Vec<String> = if o.types.is_empty() {
        DEFAULT_TYPES.iter().map(|t| (*t).to_owned()).collect()
    } else {
        o.types.clone()
    };
    let mut files = vec![
        (
            "index.md".to_owned(),
            format!(
                "---\nokf_version: \"0.2\"\n---\n# {}\n\n{}\n",
                o.title,
                if o.description.is_empty() {
                    "Documents are listed below as they are added."
                } else {
                    &o.description
                }
            ),
        ),
        (
            "_meta/vocabulary.md".to_owned(),
            format!(
                "---\ntype: Vocabulary\ntitle: Tag vocabulary\ndescription: Canonical tags for {} and their synonyms in {}.\nterms: {{}}\n---\n\nAdd a term per topic, with synonyms in every language people use, for example:\n\n```yaml\nterms:\n  refund:\n    synonyms: [returns, hoàn tiền, 返金]\n    facet: topic\n```\n",
                o.title,
                langs.join(", ")
            ),
        ),
        (
            "log.md".to_owned(),
            format!(
                "# Update log\n\n## {}\n* **Created** with `okfkit init`.\n",
                o.today
            ),
        ),
        (
            "okfkit.toml".to_owned(),
            format!(
                "# okfkit settings for this bundle (https://github.com/okfkit/okfkit)\n[bundle]\nprofile = \"okf\"\n# Languages people write and ask in; used by fine-tuning and advice.\nlangs = [{}]\n",
                langs
                    .iter()
                    .map(|l| toml_str(l))
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
        ),
    ];
    for t in &types {
        let fields = schema_fields(t);
        files.push((
            format!("{}/{t}.md", okfkit_standard::TYPES_DIR),
            format!(
                "---\ntype: Type Schema\ntitle: {t}\ndescription: Fields every {t} document carries.\nfields:{}\n---\n\nEdit the fields to match how your {t} documents are used (types: string, number, integer, bool, date, list, map, any).\n",
                if fields.is_empty() {
                    " {}\n".to_owned()
                } else {
                    format!("\n{fields}")
                }
                .trim_end_matches('\n')
            ),
        ));
    }
    Ok(files)
}

/// What `new` creates.
#[derive(Debug, Clone)]
pub struct NewOptions {
    /// Concept type.
    pub concept_type: String,
    /// Title.
    pub title: String,
    /// One-sentence description (an empty one is reported by lint).
    pub description: String,
    /// Folder (bundle-relative) [default: derived from the type].
    pub dir: Option<String>,
    /// Tags.
    pub tags: Vec<String>,
    /// Language [default: the first of `okfkit.toml` langs, else en].
    pub lang: Option<String>,
    /// Source documents this one is distilled from (bundle-relative paths).
    pub sources: Vec<String>,
    /// Today (`YYYY-MM-DD`).
    pub today: String,
}

/// A planned new document.
#[derive(Debug, Clone, serde::Serialize)]
pub struct NewDoc {
    /// Bundle-relative path.
    pub path: String,
    /// Its content.
    pub content: String,
    /// Folder listings to create or update: (path, content). Empty under the docs-site and vault
    /// profiles, which have none.
    pub index: Vec<(String, String)>,
    /// Required fields of the type's schema still to fill.
    pub to_fill: Vec<String>,
}

fn slug(s: &str) -> String {
    let folded = okfkit_analyze::fold(s);
    let mut out = String::new();
    for c in folded.chars() {
        if c.is_ascii_alphanumeric() {
            out.push(c.to_ascii_lowercase());
        } else if !out.ends_with('-') && !out.is_empty() {
            out.push('-');
        }
    }
    let out = out.trim_end_matches('-').to_owned();
    if out.is_empty() {
        "untitled".into()
    } else {
        out
    }
}

/// The default folder for a type: `Policy` → `policies`, `FAQ` → `faq`, `Guide` → `guides`.
pub fn type_dir(ty: &str) -> String {
    let s = slug(ty);
    if s == "faq" || s.ends_with('s') {
        s
    } else if let Some(stem) = s.strip_suffix('y') {
        format!("{stem}ies")
    } else {
        format!("{s}s")
    }
}

fn configured_langs(root: &Path) -> Vec<String> {
    let Ok(text) = std::fs::read_to_string(root.join("okfkit.toml")) else {
        return Vec::new();
    };
    // `langs = ["vi", "en"]` under [bundle]; a light read is enough here.
    text.lines()
        .find_map(|l| l.trim().strip_prefix("langs").map(str::trim))
        .and_then(|l| l.strip_prefix('='))
        .map(|l| {
            l.trim()
                .trim_matches(|c| c == '[' || c == ']')
                .split(',')
                .map(|x| x.trim().trim_matches('"').to_owned())
                .filter(|x| !x.is_empty())
                .collect()
        })
        .unwrap_or_default()
}

/// Plans a new document (writes nothing).
pub fn new_doc(root: &Path, o: &NewOptions) -> Result<NewDoc, Error> {
    let dir = o
        .dir
        .clone()
        .unwrap_or_else(|| type_dir(&o.concept_type))
        .trim_matches('/')
        .to_owned();
    let file = format!("{}.md", slug(&o.title));
    let path = if dir.is_empty() {
        file.clone()
    } else {
        format!("{dir}/{file}")
    };
    if root.join(&path).exists() {
        return Err(Error::Invalid(format!(
            "{path} already exists; pick another title or --dir"
        )));
    }
    let lang = o
        .lang
        .clone()
        .or_else(|| configured_langs(root).into_iter().next())
        .unwrap_or_else(|| "en".into());
    let schemas = okfkit_standard::load_type_schemas(root).unwrap_or_default();
    let to_fill: Vec<String> = schemas
        .get(&o.concept_type)
        .map(|s| {
            s.fields
                .iter()
                .filter(|(_, f)| f.required)
                .map(|(k, _)| k.clone())
                .collect()
        })
        .unwrap_or_default();
    let mut fm = format!(
        "---\ntype: {}\ntitle: {}\ndescription: {}\nlang: {lang}\nstatus: draft\nupdated: {}\n",
        yaml_str(&o.concept_type),
        yaml_str(&o.title),
        yaml_str(&o.description),
        o.today
    );
    if !o.tags.is_empty() {
        let _ = writeln!(
            fm,
            "tags: [{}]",
            o.tags
                .iter()
                .map(|t| yaml_str(t))
                .collect::<Vec<_>>()
                .join(", ")
        );
    }
    if !o.sources.is_empty() {
        let _ = writeln!(
            fm,
            "sources: [{}]",
            o.sources
                .iter()
                .map(|t| yaml_str(t))
                .collect::<Vec<_>>()
                .join(", ")
        );
    }
    for f in &to_fill {
        let _ = writeln!(
            fm,
            "{f}: \"\"  # required by _meta/types/{}.md",
            o.concept_type
        );
    }
    let content = format!("{fm}---\n# {}\n\n", o.title);
    let mut index = Vec::new();
    if !okfkit_core::site::profile(root).content_index() {
        let listing = |d: &str| {
            if d.is_empty() {
                "index.md".to_owned()
            } else {
                format!("{d}/index.md")
            }
        };
        let append = |path: &str, entry: &str, heading: &str| -> (String, String) {
            let text = match std::fs::read_to_string(root.join(path)) {
                Ok(t) => format!("{t}{}{entry}", if t.ends_with('\n') { "" } else { "\n" }),
                Err(_) => format!("# {heading}\n\n{entry}"),
            };
            (path.to_owned(), text)
        };
        let entry = format!(
            "* [{}]({file}){}\n",
            o.title,
            if o.description.is_empty() {
                String::new()
            } else {
                format!(" - {}", o.description)
            }
        );
        let own = listing(&dir);
        let new_dir = !root.join(&own).exists();
        index.push(append(&own, &entry, dir.rsplit('/').next().unwrap_or(&dir)));
        // A new folder is listed in its parent's index.
        if new_dir && !dir.is_empty() {
            let (parent, name) = dir.rsplit_once('/').unwrap_or(("", &dir));
            index.push(append(
                &listing(parent),
                &format!("* [{name}]({name}/index.md)\n"),
                if parent.is_empty() { "Index" } else { parent },
            ));
        }
    }
    Ok(NewDoc {
        path,
        content,
        index,
        to_fill,
    })
}

/// Writes files that must not exist yet (and index updates), creating folders.
pub fn write_new(
    root: &Path,
    files: &[(String, String)],
    overwrite: &[String],
) -> Result<Vec<PathBuf>, Error> {
    for (rel, _) in files {
        let p = root.join(rel);
        if p.exists() && !overwrite.contains(rel) {
            return Err(Error::Invalid(format!("{} already exists", p.display())));
        }
    }
    let mut out = Vec::new();
    for (rel, text) in files {
        let p = root.join(rel);
        if let Some(parent) = p.parent() {
            std::fs::create_dir_all(parent).map_err(io(parent))?;
        }
        std::fs::write(&p, text).map_err(io(&p))?;
        out.push(p);
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn init_then_new_reaches_l2() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        let files = init_files(
            root,
            &InitOptions {
                title: "Support knowledge".into(),
                description: "How we help customers.".into(),
                langs: vec!["vi".into(), "en".into()],
                types: vec![],
                today: "2026-10-02".into(),
            },
        )
        .unwrap();
        write_new(root, &files, &[]).unwrap();
        assert!(root.join("_meta/types/Policy.md").is_file());
        let toml = std::fs::read_to_string(root.join("okfkit.toml")).unwrap();
        assert!(toml.parse::<toml_edit::DocumentMut>().is_ok(), "{toml}");
        assert!(toml.contains(r#"langs = ["vi", "en"]"#), "{toml}");
        let doc = new_doc(
            root,
            &NewOptions {
                concept_type: "Policy".into(),
                title: "Chính sách đổi trả".into(),
                description: "Khách được đổi trả trong 30 ngày.".into(),
                dir: None,
                tags: vec![],
                lang: None,
                sources: vec!["manuals/returns.pdf".into()],
                today: "2026-10-02".into(),
            },
        )
        .unwrap();
        assert_eq!(doc.path, "policies/chinh-sach-doi-tra.md");
        assert_eq!(doc.to_fill, ["owner"]);
        assert!(
            doc.content.contains("lang: vi"),
            "first configured language"
        );
        let mut files = vec![(
            doc.path.clone(),
            doc.content.replace("owner: \"\"", "owner: \"cs-team\""),
        )];
        files.extend(doc.index.clone());
        let root_index = std::fs::read_to_string(root.join("index.md")).unwrap();
        write_new(root, &files, &["index.md".to_owned()]).unwrap();
        assert!(
            files
                .iter()
                .any(|(p, t)| p == "index.md" && t.contains("* [policies](policies/index.md)")),
            "{root_index}"
        );
        assert!(
            doc.content
                .starts_with("---\ntype: Policy\ntitle: Chính sách đổi trả\n"),
            "{}",
            doc.content
        );
        // The bundle is valid OKF; with the vocabulary empty, L1 is fully met.
        let a = okfkit_standard::assess_bundle(root, okfkit_standard::VOCABULARY_PATH).unwrap();
        assert!(
            a.level >= Some(okfkit_standard::Level::L1),
            "{:#?}",
            a.findings
        );
        // init refuses a folder with documents; new refuses an existing path.
        assert!(
            init_files(
                root,
                &InitOptions {
                    title: "x".into(),
                    description: String::new(),
                    langs: vec![],
                    types: vec![],
                    today: "x".into()
                }
            )
            .is_err()
        );
        assert!(
            new_doc(
                root,
                &NewOptions {
                    concept_type: "Policy".into(),
                    title: "Chính sách đổi trả".into(),
                    description: String::new(),
                    dir: None,
                    tags: vec![],
                    lang: None,
                    sources: vec![],
                    today: "x".into()
                }
            )
            .is_err()
        );
        assert_eq!(type_dir("Guide"), "guides");
        assert_eq!(type_dir("Policy"), "policies");
        assert_eq!(type_dir("FAQ"), "faq");
    }
}
