//! Model-free guesses: site kind, `type`, `title`, `description`, `lang`.

use std::path::Path;
use std::sync::LazyLock;

use regex::Regex;
use serde::Serialize;

/// The kind of documentation site a folder looks like.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Site {
    /// An existing OKF bundle (root `index.md` with `okf_version`, or most files with `type`).
    Okf,
    /// An Obsidian vault (`.obsidian/`).
    Obsidian,
    /// Docusaurus (`docusaurus.config.*`).
    Docusaurus,
    /// Hugo (`hugo.toml`/`config.toml` with `content/`).
    Hugo,
    /// Mintlify (`mint.json` or `docs.json`).
    Mintlify,
    /// MkDocs (`mkdocs.yml`).
    Mkdocs,
    /// A plain folder of markdown files.
    Plain,
}

/// Recognizes the site kind from marker files at the root.
pub fn detect_site(root: &Path) -> Site {
    let has = |p: &str| root.join(p).exists();
    if has(".obsidian") {
        Site::Obsidian
    } else if [
        "docusaurus.config.js",
        "docusaurus.config.ts",
        "docusaurus.config.mjs",
    ]
    .iter()
    .any(|p| has(p))
    {
        Site::Docusaurus
    } else if has("mint.json") || has("docs.json") {
        Site::Mintlify
    } else if has("mkdocs.yml") || has("mkdocs.yaml") {
        Site::Mkdocs
    } else if (has("hugo.toml") || has("config.toml")) && has("content") {
        Site::Hugo
    } else if std::fs::read_to_string(root.join("index.md"))
        .is_ok_and(|t| t.contains("okf_version"))
    {
        Site::Okf
    } else {
        Site::Plain
    }
}

/// Directory name → concept type. The first matching path segment (deepest first) wins.
const DIR_TYPES: &[(&[&str], &str)] = &[
    (&["faq", "faqs", "help"], "FAQ"),
    (&["adr", "adrs", "decisions"], "Decision"),
    (
        &[
            "tutorial",
            "tutorials",
            "start",
            "getting-started",
            "quickstart",
        ],
        "Tutorial",
    ),
    (&["guide", "guides", "howto", "how-to", "how-tos"], "Guide"),
    (&["reference", "references", "api", "cli"], "Reference"),
    (&["concept", "concepts", "architecture"], "Concept"),
    (&["policy", "policies"], "Policy"),
    (
        &[
            "sop",
            "sops",
            "runbook",
            "runbooks",
            "playbook",
            "playbooks",
        ],
        "Playbook",
    ),
    (&["blog", "posts", "news", "announcements"], "Post"),
    (&["changelog", "releases", "release-notes"], "Release Notes"),
    (&["meeting", "meetings", "minutes"], "Meeting Note"),
    (&["spec", "specs", "rfc", "rfcs"], "Spec"),
    (&["install", "installation", "setup"], "Install Guide"),
    (&["troubleshooting"], "Troubleshooting"),
];

/// Guesses `type` from the file name and directories; `Document` when nothing matches.
pub fn guess_type(id: &str) -> &'static str {
    let name = id.rsplit('/').next().unwrap_or(id).to_ascii_lowercase();
    if name.starts_with("adr-") || name.starts_with("adr_") {
        return "Decision";
    }
    if name == "faq" || name.ends_with("-faq") {
        return "FAQ";
    }
    if name.contains("changelog") {
        return "Release Notes";
    }
    let segments: Vec<String> = id.split('/').map(str::to_ascii_lowercase).collect();
    for seg in segments.iter().rev().skip(1) {
        if let Some((_, ty)) = DIR_TYPES
            .iter()
            .find(|(names, _)| names.contains(&seg.as_str()))
        {
            return ty;
        }
    }
    "Document"
}

/// `getting-started_guide` → `Getting started guide`.
pub fn title_from_name(name: &str) -> String {
    let words = name.replace(['-', '_'], " ");
    let mut c = words.trim().chars();
    match c.next() {
        Some(f) => f.to_uppercase().chain(c).collect(),
        None => name.to_owned(),
    }
}

/// The first `# ` heading outside code fences.
pub fn first_h1(body: &str) -> Option<String> {
    let mut fenced = false;
    for line in body.lines() {
        let t = line.trim_start();
        if t.starts_with("```") || t.starts_with("~~~") {
            fenced = !fenced;
        }
        if !fenced && let Some(h) = line.strip_prefix("# ") {
            let h = clean_inline(h.trim().trim_end_matches('#').trim());
            if !h.is_empty() {
                return Some(h);
            }
        }
    }
    None
}

static LINK: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"!?\[([^\]]*)\]\([^)]*\)").expect("valid regex"));
static REF_LINK: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\[([^\]]+)\]\[[^\]]*\]").expect("valid regex"));
static WIKI: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"\[\[([^\]|#]+)(?:#[^\]|]*)?(?:\|([^\]]+))?\]\]").expect("valid regex")
});
static HTML: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"<[^>]+>").expect("valid regex"));
static EMPH: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(\*\*|__|\*|_|~~)([^*_~]+)(\*\*|__|\*|_|~~)").expect("valid regex")
});
static FOOTNOTE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\[\^[^\]]*\]").expect("valid regex"));

/// Markdown inline syntax → plain text.
pub fn clean_inline(s: &str) -> String {
    let s = WIKI.replace_all(s, |c: &regex::Captures| {
        c.get(2).or(c.get(1)).map_or("", |m| m.as_str()).to_owned()
    });
    let s = LINK.replace_all(&s, "$1");
    let s = REF_LINK.replace_all(&s, "$1");
    let s = FOOTNOTE.replace_all(&s, "");
    let s = HTML.replace_all(&s, "");
    let s = EMPH.replace_all(&s, "$2");
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Longest generated description, in characters.
pub const MAX_DESCRIPTION: usize = 200;
/// Shortest generated description; shorter first sentences are extended with the next one.
pub const MIN_DESCRIPTION: usize = 40;

/// The first meaningful sentence of a body: prose paragraphs first, then list items.
/// Skips headings, code, tables, HTML/JSX, MDX imports, admonitions and images.
pub fn first_sentence(body: &str) -> Option<String> {
    let mut paragraphs: Vec<(bool, String)> = Vec::new(); // (is_list, text)
    let mut current = String::new();
    let mut fenced = false;
    let mut admonition = false;
    let mut html_depth = 0i32;
    let flush = |current: &mut String, list: bool, out: &mut Vec<(bool, String)>| {
        let t = clean_inline(current);
        if t.chars().filter(|c| c.is_alphabetic()).count() >= 8 {
            out.push((list, t));
        }
        current.clear();
    };
    for line in body.lines() {
        let t = line.trim();
        if t.starts_with("```") || t.starts_with("~~~") {
            fenced = !fenced;
            flush(&mut current, false, &mut paragraphs);
            continue;
        }
        if fenced {
            continue;
        }
        if t.starts_with(":::") {
            admonition = !admonition && t.len() > 3;
            flush(&mut current, false, &mut paragraphs);
            continue;
        }
        if admonition {
            continue;
        }
        // Multi-line HTML/JSX blocks such as <p align="center"> … </p> or <Card …/>.
        if t.starts_with('<') && !t.starts_with("<http") {
            let opens = t.matches('<').count() as i32
                - t.matches("</").count() as i32 * 2
                - t.matches("/>").count() as i32;
            html_depth = (html_depth + opens).max(0);
            flush(&mut current, false, &mut paragraphs);
            continue;
        }
        if html_depth > 0 {
            if t.starts_with("</") || t.ends_with("/>") || t.ends_with('>') {
                html_depth = (html_depth - 1).max(0);
            }
            continue;
        }
        let list_item =
            t.starts_with("- ") || t.starts_with("* ") || t.starts_with("+ ") || starts_numbered(t);
        let skip = t.is_empty()
            || t.starts_with('#')
            || t.starts_with('|')
            || t.starts_with('>')
            || t.starts_with("![")
            || t.starts_with("[![")
            || t.starts_with("import ")
            || t.starts_with("export ")
            || t.starts_with("{/*")
            || t.starts_with("---")
            || t.starts_with("===");
        if skip || list_item {
            flush(&mut current, false, &mut paragraphs);
        }
        if skip {
            continue;
        }
        if list_item {
            current.push_str(
                t.trim_start_matches(['-', '*', '+', ' '])
                    .trim_start_matches(|c: char| c.is_ascii_digit() || c == '.' || c == ')'),
            );
            flush(&mut current, true, &mut paragraphs);
            continue;
        }
        if !current.is_empty() {
            current.push(' ');
        }
        current.push_str(t);
    }
    flush(&mut current, false, &mut paragraphs);
    let text = paragraphs
        .iter()
        .find(|(list, _)| !list)
        .or(paragraphs.first())
        .map(|(_, t)| t.clone())?;
    Some(sentence(&text))
}

fn starts_numbered(t: &str) -> bool {
    let digits = t.chars().take_while(char::is_ascii_digit).count();
    digits > 0 && (t[digits..].starts_with(". ") || t[digits..].starts_with(") "))
}

/// The first sentence (or two, if the first is very short), at most `MAX_DESCRIPTION` characters.
fn sentence(text: &str) -> String {
    let chars: Vec<char> = text.chars().collect();
    let mut end = chars.len();
    let mut i = 0;
    let mut weight = 0; // CJK characters count triple, as in okfkit-lint
    while i < chars.len() {
        let c = chars[i];
        weight += if okfkit_analyze::is_cjk(c) { 3 } else { 1 };
        let next = chars.get(i + 1).copied();
        let boundary = matches!(c, '。' | '！' | '？')
            || (matches!(c, '.' | '!' | '?')
                && next.is_none_or(char::is_whitespace)
                && !abbreviation(&chars[..i]));
        if boundary && weight >= MIN_DESCRIPTION.min(chars.len()) {
            end = i + 1;
            break;
        }
        i += 1;
    }
    let s: String = chars[..end].iter().collect();
    let s = s.trim().to_owned();
    if s.chars().count() <= MAX_DESCRIPTION {
        return s;
    }
    let cut: String = s.chars().take(MAX_DESCRIPTION - 1).collect();
    let cut = cut
        .rsplit_once(' ')
        .map_or(cut.as_str(), |(a, _)| a)
        .trim_end_matches([',', ';', ':']);
    format!("{cut}…")
}

fn abbreviation(before: &[char]) -> bool {
    let word: String = before
        .iter()
        .rev()
        .take_while(|c| !c.is_whitespace())
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect();
    let w = word.to_ascii_lowercase();
    [
        "e.g", "i.e", "etc", "vs", "approx", "no", "fig", "mr", "dr", "ms",
    ]
    .contains(&w.trim_start_matches('('))
        || w.len() == 1
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn types() {
        assert_eq!(guess_type("help/faq"), "FAQ");
        assert_eq!(guess_type("docs/guides/linux"), "Guide");
        assert_eq!(guess_type("docs/guides/setup/linux"), "Install Guide"); // deepest directory wins
        assert_eq!(guess_type("decisions/adr-001-db"), "Decision");
        assert_eq!(guess_type("gateway/config"), "Document");
        assert_eq!(guess_type("cli/agents"), "Reference");
    }

    #[test]
    fn titles() {
        assert_eq!(
            title_from_name("getting-started_guide"),
            "Getting started guide"
        );
        assert_eq!(
            first_h1("```\n# not\n```\n# OpenClaw 🦞\n").as_deref(),
            Some("OpenClaw 🦞")
        );
        assert_eq!(
            first_h1("# The [API](x.md) **guide**").as_deref(),
            Some("The API guide")
        );
    }

    #[test]
    fn descriptions() {
        let body = "import X from './x';\n\n# Title\n\n<p align=\"center\">\n  <img src=\"a.png\"/>\n</p>\n\n:::note\nSkip me please.\n:::\n\n![badge](b.svg)\n\nActive Memory is the deep-recall lane for eligible sessions. It runs only when needed.\n";
        assert_eq!(
            first_sentence(body).as_deref(),
            Some("Active Memory is the deep-recall lane for eligible sessions.")
        );
        let short =
            "Use it. Then configure the gateway with the `openclaw config` command and restart.";
        assert_eq!(first_sentence(short).as_deref(), Some(short));
        assert_eq!(
            first_sentence(
                "Chính sách đổi trả áp dụng cho mọi đơn hàng trong vòng 30 ngày. Xem thêm."
            )
            .as_deref(),
            Some("Chính sách đổi trả áp dụng cho mọi đơn hàng trong vòng 30 ngày.")
        );
        assert_eq!(
            first_sentence("返品は商品到着後30日以内に受け付けます。詳しくは下記をご覧ください。")
                .as_deref(),
            Some("返品は商品到着後30日以内に受け付けます。")
        );
        assert_eq!(
            first_sentence("- Install the CLI with npm, e.g. npm i -g openclaw.\n- Run it.")
                .as_deref(),
            Some("Install the CLI with npm, e.g. npm i -g openclaw.")
        );
        let long = "word ".repeat(80);
        let d = first_sentence(&long).unwrap();
        assert!(d.chars().count() <= MAX_DESCRIPTION && d.ends_with('…'));
        assert_eq!(first_sentence("# Only a heading\n\n```\ncode\n```\n"), None);
        assert_eq!(
            clean_inline("See [[Orders|the orders]] and [x](y.md) **now**"),
            "See the orders and x now"
        );
    }
}
