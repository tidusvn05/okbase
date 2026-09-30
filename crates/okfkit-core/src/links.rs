//! Link extraction (markdown links and wikilinks) and resolution to concept IDs.

use pulldown_cmark::{Event, LinkType, Options, Parser, Tag};

use crate::ConceptId;

/// The syntax a link was written in.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum LinkKind {
    /// A standard markdown link or image: `[text](target)`.
    Markdown,
    /// A wikilink: `[[target]]` or `[[target|text]]`.
    Wiki,
}

/// A link found in a concept body.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct Link {
    /// The target as written (for wikilinks, without the `|alias` part).
    pub target: String,
    /// The link text.
    pub text: String,
    /// The link syntax.
    pub kind: LinkKind,
    /// 1-based line number in the body.
    pub line: usize,
}

/// Extracts links from a markdown body, ignoring code spans and code blocks.
pub fn extract_links(body: &str) -> Vec<Link> {
    let opts = Options::ENABLE_WIKILINKS | Options::ENABLE_TABLES | Options::ENABLE_FOOTNOTES;
    let mut out = Vec::new();
    let mut open: Vec<(usize, Link)> = Vec::new();
    for (event, range) in Parser::new_ext(body, opts).into_offset_iter() {
        match event {
            Event::Start(
                Tag::Link {
                    link_type,
                    dest_url,
                    ..
                }
                | Tag::Image {
                    link_type,
                    dest_url,
                    ..
                },
            ) => {
                let kind = if matches!(link_type, LinkType::WikiLink { .. }) {
                    LinkKind::Wiki
                } else {
                    LinkKind::Markdown
                };
                let line = body[..range.start].matches('\n').count() + 1;
                open.push((
                    out.len(),
                    Link {
                        target: dest_url.to_string(),
                        text: String::new(),
                        kind,
                        line,
                    },
                ));
            }
            Event::Text(t) | Event::Code(t) => {
                if let Some((_, link)) = open.last_mut() {
                    link.text.push_str(&t);
                }
            }
            Event::End(pulldown_cmark::TagEnd::Link | pulldown_cmark::TagEnd::Image) => {
                if let Some((_, link)) = open.pop() {
                    out.push(link);
                }
            }
            _ => {}
        }
    }
    out
}

/// Resolves a markdown link target written in concept `from` to a concept ID.
///
/// Handles bundle-absolute (`/a/b.md`) and relative (`../b.md`) paths, strips
/// `#fragment` and `?query`, decodes `%XX`, and maps a directory (`dir/`) to its
/// `dir/index`. Returns `None` for external URLs, pure anchors, non-markdown
/// targets and paths that leave the bundle.
pub fn resolve_link(from: &ConceptId, target: &str) -> Option<ConceptId> {
    let target = target.trim();
    let target = target.split(['#', '?']).next()?;
    if target.is_empty() || has_scheme(target) || target.starts_with("//") {
        return None;
    }
    let target = percent_decode(target);
    let joined = match target.strip_prefix('/') {
        Some(abs) => abs.to_owned(),
        None if from.dir().is_empty() => target.clone(),
        None => format!("{}/{}", from.dir(), target),
    };
    let mut parts: Vec<&str> = Vec::new();
    for seg in joined.split('/') {
        match seg {
            "" | "." => {}
            ".." => {
                parts.pop()?;
            }
            s => parts.push(s),
        }
    }
    let path = if joined.ends_with('/') || parts.is_empty() {
        parts.push("index");
        parts.join("/")
    } else {
        parts.join("/").strip_suffix(".md")?.to_owned()
    };
    ConceptId::new(path).ok()
}

/// Resolves a wikilink target against the known concept IDs: an exact ID
/// (with or without `.md`) first, then a unique concept with that file name.
pub fn resolve_wikilink<'a>(
    target: &str,
    ids: impl IntoIterator<Item = &'a ConceptId>,
) -> Option<ConceptId> {
    let t = target.split('#').next()?.trim().trim_start_matches('/');
    let t = t.strip_suffix(".md").unwrap_or(t);
    let mut by_name = None;
    let mut ambiguous = false;
    for id in ids {
        if id.as_str() == t {
            return Some(id.clone());
        }
        if id.name() == t {
            ambiguous |= by_name.is_some();
            by_name = Some(id);
        }
    }
    if ambiguous { None } else { by_name.cloned() }
}

fn has_scheme(s: &str) -> bool {
    match s.find(':') {
        Some(i) => {
            let scheme = &s[..i];
            scheme.len() > 1
                && scheme
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || "+-.".contains(c))
        }
        None => false,
    }
}

fn percent_decode(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%'
            && let Some(v) = s
                .get(i + 1..i + 3)
                .and_then(|h| u8::from_str_radix(h, 16).ok())
        {
            out.push(v);
            i += 3;
            continue;
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8(out).unwrap_or_else(|_| s.to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn id(s: &str) -> ConceptId {
        ConceptId::new(s).unwrap()
    }

    #[test]
    fn extracts() {
        let body = "See [orders](/tables/orders.md) and [[Customers|cust]].\n\n`[x](y.md)`\n\n```\n[z](z.md)\n```\n![img](a.png)\n";
        let links = extract_links(body);
        let got: Vec<_> = links
            .iter()
            .map(|l| (l.target.as_str(), l.text.as_str(), l.kind, l.line))
            .collect();
        assert_eq!(
            got,
            [
                ("/tables/orders.md", "orders", LinkKind::Markdown, 1),
                ("Customers", "cust", LinkKind::Wiki, 1),
                ("a.png", "img", LinkKind::Markdown, 8),
            ]
        );
    }

    #[test]
    fn resolves() {
        let from = id("guides/setup");
        let r = |t| resolve_link(&from, t).map(|i| i.to_string());
        assert_eq!(r("/tables/orders.md").as_deref(), Some("tables/orders"));
        assert_eq!(r("./install.md#step-2").as_deref(), Some("guides/install"));
        assert_eq!(r("../faq/My%20Page.md").as_deref(), Some("faq/My Page"));
        assert_eq!(r("../faq/").as_deref(), Some("faq/index"));
        assert_eq!(r("../../x.md"), None);
        assert_eq!(r("https://example.com/a.md"), None);
        assert_eq!(r("mailto:a@b.c"), None);
        assert_eq!(r("#anchor"), None);
        assert_eq!(r("image.png"), None);
    }

    #[test]
    fn resolves_wikilinks() {
        let ids = [id("a/Customers"), id("b/Orders"), id("c/Orders")];
        assert_eq!(resolve_wikilink("Customers", &ids), Some(id("a/Customers")));
        assert_eq!(resolve_wikilink("b/Orders.md", &ids), Some(id("b/Orders")));
        assert_eq!(resolve_wikilink("Orders", &ids), None);
    }
}
