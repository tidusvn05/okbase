//! Read-time mapping of common non-OKF frontmatter fields to OKF fields.
//!
//! | OKF field | Also read from |
//! |---|---|
//! | `title` | `sidebar_label` (Docusaurus) |
//! | `description` | `summary` (Mintlify, OpenClaw), `excerpt` (Jekyll) |
//! | `tags` | `categories` (Jekyll, Hugo) |
//!
//! The OKF field always wins when present. Nothing is written back.

use okfkit_core::{Frontmatter, Value};
use serde::Serialize;

const TITLE_KEYS: [&str; 2] = ["title", "sidebar_label"];
const DESCRIPTION_KEYS: [&str; 3] = ["description", "summary", "excerpt"];
const TAG_KEYS: [&str; 2] = ["tags", "categories"];

/// A value together with the frontmatter key it was read from.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Mapped<T> {
    /// The value.
    pub value: T,
    /// The key it came from, such as `summary`.
    pub from: &'static str,
}

impl<T> Mapped<T> {
    /// Whether the value came from the OKF field itself rather than a foreign one.
    pub fn is_native(&self, okf_key: &str) -> bool {
        self.from == okf_key
    }
}

/// The OKF view of a document's frontmatter, with foreign fields mapped in.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct Meta {
    /// `type`, if it is a non-empty string.
    pub concept_type: Option<String>,
    /// Display title.
    pub title: Option<Mapped<String>>,
    /// One-line description.
    pub description: Option<Mapped<String>>,
    /// Tags as written (not normalized).
    pub tags: Option<Mapped<Vec<String>>>,
}

/// Computes the OKF view of a frontmatter block.
pub fn meta(fm: &Frontmatter) -> Meta {
    Meta {
        concept_type: non_empty(fm.get("type")),
        title: first(fm, &TITLE_KEYS, non_empty),
        description: first(fm, &DESCRIPTION_KEYS, non_empty),
        tags: first(fm, &TAG_KEYS, string_list),
    }
}

fn first<T>(
    fm: &Frontmatter,
    keys: &[&'static str],
    read: impl Fn(Option<&Value>) -> Option<T>,
) -> Option<Mapped<T>> {
    keys.iter()
        .find_map(|&k| read(fm.get(k)).map(|value| Mapped { value, from: k }))
}

fn non_empty(v: Option<&Value>) -> Option<String> {
    let s = v?.as_str()?.trim();
    (!s.is_empty()).then(|| s.to_owned())
}

/// A list of strings, or a single string (`tags: a` or Jekyll's `categories: a b`).
fn string_list(v: Option<&Value>) -> Option<Vec<String>> {
    let items: Vec<String> = match v? {
        Value::Array(items) => items
            .iter()
            .filter_map(|i| match i {
                Value::String(s) => Some(s.trim().to_owned()),
                Value::Number(n) => Some(n.to_string()),
                _ => None,
            })
            .filter(|s| !s.is_empty())
            .collect(),
        Value::String(s) if s.contains(',') => s
            .split(',')
            .map(|t| t.trim().to_owned())
            .filter(|t| !t.is_empty())
            .collect(),
        Value::String(s) => s.split_whitespace().map(str::to_owned).collect(),
        _ => return None,
    };
    (!items.is_empty()).then_some(items)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn m(yaml: &str) -> Meta {
        meta(&Frontmatter::split(&format!("---\n{yaml}---\n")).0)
    }

    #[test]
    fn maps_foreign_fields() {
        let got =
            m("summary: Set up the gateway.\nsidebar_label: Setup\ncategories: [ops, gateway]\n");
        assert_eq!(
            got.title,
            Some(Mapped {
                value: "Setup".into(),
                from: "sidebar_label"
            })
        );
        assert_eq!(
            got.description,
            Some(Mapped {
                value: "Set up the gateway.".into(),
                from: "summary"
            })
        );
        assert_eq!(got.tags.unwrap().value, ["ops", "gateway"]);
        assert_eq!(got.concept_type, None);
    }

    #[test]
    fn native_fields_win() {
        let got = m(
            "type: Guide\ntitle: T\nsidebar_label: S\ndescription: D\nsummary: X\nexcerpt: E\ntags: a, b\n",
        );
        assert!(got.title.unwrap().is_native("title"));
        assert_eq!(got.description.unwrap().value, "D");
        assert_eq!(got.tags.unwrap().value, ["a", "b"]);
        assert_eq!(got.concept_type.as_deref(), Some("Guide"));
    }

    #[test]
    fn empty_values_fall_through() {
        let got = m("description: ''\nexcerpt: Real\ntags: []\ncategories: jekyll hugo\n");
        assert_eq!(got.description.unwrap().from, "excerpt");
        assert_eq!(got.tags.unwrap().value, ["jekyll", "hugo"]);
    }
}
