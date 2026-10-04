//! Markdown → HTML for the viewer.
//!
//! Raw HTML in documents is shown as text (never passed through), links to other
//! documents point at the viewer's `#/doc/<id>` route using the targets the index
//! already resolved, and relative images are served from `/files/`.

use std::collections::HashMap;

use pulldown_cmark::{CowStr, Event, HeadingLevel, Options, Parser, Tag, TagEnd, html};
use serde::Serialize;

use okbase::LinksResult;

/// A heading of the rendered document, for the table of contents.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Heading {
    /// 1–6.
    pub level: u8,
    /// Heading text.
    pub text: String,
    /// `id` attribute of the heading element.
    pub anchor: String,
}

/// Renders `body` (the markdown of the document at bundle-relative `path`) to HTML.
pub fn render(body: &str, path: &str, links: &LinksResult) -> (String, Vec<Heading>) {
    // Targets as written → visible document id (None: missing or hidden).
    let mut targets: HashMap<&str, Option<&str>> = HashMap::new();
    for l in &links.outgoing {
        let id = if l.exists { l.id.as_deref() } else { None };
        targets.insert(l.raw.as_str(), id);
    }
    let opts = Options::ENABLE_TABLES
        | Options::ENABLE_FOOTNOTES
        | Options::ENABLE_STRIKETHROUGH
        | Options::ENABLE_TASKLISTS
        | Options::ENABLE_WIKILINKS;
    let mut events: Vec<Event<'_>> = Parser::new_ext(body, opts).collect();

    // Give every heading an anchor and collect the table of contents.
    let mut headings = Vec::new();
    let mut used: HashMap<String, usize> = HashMap::new();
    let mut i = 0;
    while i < events.len() {
        if let Event::Start(Tag::Heading { level, .. }) = &events[i] {
            let level = heading_level(*level);
            let mut text = String::new();
            let mut j = i + 1;
            while j < events.len() && !matches!(events[j], Event::End(TagEnd::Heading(_))) {
                if let Event::Text(t) | Event::Code(t) = &events[j] {
                    text.push_str(t);
                }
                j += 1;
            }
            let mut anchor = slug(&text);
            let n = used.entry(anchor.clone()).or_default();
            *n += 1;
            if *n > 1 {
                anchor = format!("{anchor}-{n}");
            }
            if let Event::Start(Tag::Heading { id, .. }) = &mut events[i] {
                *id = Some(CowStr::from(anchor.clone()));
            }
            headings.push(Heading {
                level,
                text,
                anchor,
            });
        }
        i += 1;
    }

    let dir = path.rsplit_once('/').map_or("", |(d, _)| d);
    // External images become links: the viewer never makes the browser contact other hosts.
    let mut image_is_link: Vec<bool> = Vec::new();
    let events = events.into_iter().map(|ev| match ev {
        Event::Html(h) | Event::InlineHtml(h) => Event::Text(h),
        Event::Start(Tag::Link {
            link_type,
            dest_url,
            title,
            id,
        }) => {
            let dest = link_href(&dest_url, &targets);
            Event::Start(Tag::Link {
                link_type,
                dest_url: CowStr::from(dest),
                title,
                id,
            })
        }
        Event::Start(Tag::Image {
            link_type,
            dest_url,
            title,
            id,
        }) => {
            if is_external(&dest_url) {
                image_is_link.push(true);
                return Event::Start(Tag::Link {
                    link_type,
                    dest_url,
                    title,
                    id,
                });
            }
            image_is_link.push(false);
            let dest = if dest_url.starts_with("data:") {
                dest_url.to_string()
            } else {
                match join(dir, &dest_url) {
                    Some(p) => format!("/files/{}", encode_path(&p)),
                    None => String::new(),
                }
            };
            Event::Start(Tag::Image {
                link_type,
                dest_url: CowStr::from(dest),
                title,
                id,
            })
        }
        Event::End(TagEnd::Image) if image_is_link.pop() == Some(true) => Event::End(TagEnd::Link),
        ev => ev,
    });
    let mut out = String::with_capacity(body.len() * 3 / 2);
    html::push_html(&mut out, events);
    (out, headings)
}

fn link_href(dest: &str, targets: &HashMap<&str, Option<&str>>) -> String {
    if let Some(id) = targets.get(dest) {
        return match id {
            Some(id) => format!("#/doc/{}", encode_path(id)),
            None => "#/missing".to_owned(),
        };
    }
    if is_external(dest) {
        return dest.to_owned();
    }
    if dest.starts_with('#') {
        // In-page anchor: the viewer scrolls to it (routes start with `#/`).
        return dest.to_owned();
    }
    // A link the index does not track (an asset, a file outside the bundle): leave it inert.
    "#/missing".to_owned()
}

fn is_external(s: &str) -> bool {
    let lower = s.trim_start().to_ascii_lowercase();
    ["http://", "https://", "mailto:"]
        .iter()
        .any(|p| lower.starts_with(p))
}

/// Joins a relative target to the document's directory; `None` when it leaves the bundle.
pub(crate) fn join(dir: &str, target: &str) -> Option<String> {
    let target = target.split(['#', '?']).next().unwrap_or("");
    let target = percent_decode(target);
    let mut parts: Vec<&str> = Vec::new();
    let start = if let Some(abs) = target.strip_prefix('/') {
        abs
    } else {
        parts.extend(dir.split('/').filter(|p| !p.is_empty()));
        target.as_str()
    };
    for p in start.split('/') {
        match p {
            "" | "." => {}
            ".." => {
                parts.pop()?;
            }
            p => parts.push(p),
        }
    }
    if parts.is_empty() {
        return None;
    }
    Some(parts.join("/"))
}

fn percent_decode(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%'
            && i + 2 < bytes.len()
            && let Ok(b) = u8::from_str_radix(&s[i + 1..i + 3], 16)
        {
            out.push(b);
            i += 3;
            continue;
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// Percent-encodes a path for a URL, keeping `/`.
pub(crate) fn encode_path(p: &str) -> String {
    let mut out = String::with_capacity(p.len());
    for b in p.bytes() {
        if b.is_ascii_alphanumeric() || b"-_.~/".contains(&b) {
            out.push(b as char);
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
    out
}

fn heading_level(l: HeadingLevel) -> u8 {
    match l {
        HeadingLevel::H1 => 1,
        HeadingLevel::H2 => 2,
        HeadingLevel::H3 => 3,
        HeadingLevel::H4 => 4,
        HeadingLevel::H5 => 5,
        HeadingLevel::H6 => 6,
    }
}

fn slug(text: &str) -> String {
    let mut s = String::new();
    let mut dash = false;
    for c in text.chars().flat_map(char::to_lowercase) {
        if c.is_alphanumeric() {
            s.push(c);
            dash = false;
        } else if !dash && !s.is_empty() {
            s.push('-');
            dash = true;
        }
    }
    let s = s.trim_end_matches('-').to_owned();
    if s.is_empty() { "section".into() } else { s }
}

#[cfg(test)]
mod tests {
    use super::*;
    use okbase_query::LinkRow;

    fn links(rows: &[(&str, Option<&str>, bool)]) -> LinksResult {
        LinksResult {
            id: "a/doc".into(),
            outgoing: rows
                .iter()
                .map(|(raw, id, exists)| LinkRow {
                    id: id.map(str::to_owned),
                    raw: (*raw).to_owned(),
                    text: String::new(),
                    kind: "markdown".into(),
                    line: 1,
                    exists: *exists,
                })
                .collect(),
            backlinks: vec![],
        }
    }

    #[test]
    fn rewrites_links_and_escapes_html() {
        let body = "# Title\n\nSee [b](b.md), [[Gone]], [web](https://x.org) and [c](../c.md).\n\n<script>alert(1)</script>\n\n![img](../img/p.png) ![remote](https://x.org/p.png)\n\n## Title\n";
        let l = links(&[
            ("b.md", Some("a/b"), true),
            ("Gone", None, false),
            ("../c.md", Some("c"), false),
        ]);
        let (html, toc) = render(body, "a/doc.md", &l);
        assert!(html.contains(r##"href="#/doc/a/b""##), "{html}");
        assert!(html.contains(r#"href="https://x.org""#));
        assert_eq!(html.matches(r##"href="#/missing""##).count(), 2, "{html}");
        assert!(!html.contains("<script>"), "{html}");
        assert!(html.contains("&lt;script&gt;"));
        assert!(html.contains(r#"src="/files/img/p.png""#), "{html}");
        assert!(
            html.contains(r#"<a href="https://x.org/p.png">remote</a>"#),
            "{html}"
        );
        let anchors: Vec<&str> = toc.iter().map(|h| h.anchor.as_str()).collect();
        assert_eq!(anchors, ["title", "title-2"]);
        assert!(html.contains(r#"<h1 id="title">"#));
    }

    #[test]
    fn join_stays_in_bundle() {
        assert_eq!(join("a/b", "../c.png").as_deref(), Some("a/c.png"));
        assert_eq!(join("a", "/x/y%20z.png#k").as_deref(), Some("x/y z.png"));
        assert_eq!(join("", "../escape.png"), None);
    }
}
