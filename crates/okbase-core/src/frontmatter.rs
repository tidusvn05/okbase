//! YAML frontmatter that round-trips byte for byte.
//!
//! serde cannot preserve comments or formatting, so a [`Frontmatter`] keeps the raw
//! YAML text and a parsed, ordered view of it. [`Frontmatter::set`] and
//! [`Frontmatter::remove`] edit the raw text of a single top-level key and then
//! re-parse to prove that no other key changed.

use std::sync::LazyLock;

use regex::Regex;
use serde_json::Map;

use crate::{Error, Value};

/// The parsed frontmatter mapping, in source order.
pub type Mapping = Map<String, Value>;

/// The frontmatter block of a markdown file.
///
/// A file without a frontmatter block has an absent (but editable) `Frontmatter`.
#[derive(Debug, Clone, PartialEq)]
pub struct Frontmatter {
    /// Opening delimiter line, including an optional BOM and the line ending. Empty if absent.
    open: String,
    /// YAML text between the delimiter lines. Ends with a line ending unless empty.
    raw: String,
    /// Closing delimiter line, including its line ending (if any). Empty if absent.
    close: String,
    /// Line ending used by the document.
    eol: &'static str,
    parsed: Parsed,
}

#[derive(Debug, Clone, PartialEq)]
enum Parsed {
    Map(Mapping),
    Error(String),
}

/// The state of a file's frontmatter block.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FrontmatterState {
    /// The file has no frontmatter block.
    Absent,
    /// The file starts with `---` but the block is never closed.
    Unterminated,
    /// The block is present but is not valid YAML, or is not a mapping.
    Invalid,
    /// The block is present and parses to a mapping (possibly empty).
    Valid,
}

static KEY_LINE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r#"^(?:"((?:[^"\\]|\\.)*)"|'((?:[^']|'')*)'|([^\s#'"\[\]{},&*!|>%@`?:-](?:[^:\n]|:\S)*?|-\S(?:[^:\n]|:\S)*?))[ \t]*:(?:[ \t]|\r?$)"#,
    )
    .expect("valid regex")
});

static PLAIN_KEY: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^[A-Za-z_][A-Za-z0-9_.\-]*$").expect("valid regex"));

impl Frontmatter {
    /// Splits `text` into a frontmatter block and the body that follows it.
    ///
    /// Never fails: a file without a valid block yields an absent, unterminated or
    /// invalid frontmatter and the body. `fm.render() + body == text` always holds.
    pub fn split(text: &str) -> (Frontmatter, &str) {
        let eol = if text.contains("\r\n") { "\r\n" } else { "\n" };
        let bom = if text.starts_with('\u{feff}') {
            "\u{feff}"
        } else {
            ""
        };
        let rest = &text[bom.len()..];
        let Some(first_len) = delimiter_line_len(rest, "---") else {
            return (Frontmatter::absent(eol, false), text);
        };
        let open_end = bom.len() + first_len;
        let mut pos = open_end;
        while pos < text.len() {
            let line_end = text[pos..].find('\n').map_or(text.len(), |i| pos + i + 1);
            if let Some(len) = delimiter_line_len(&text[pos..], "---") {
                let fm = Frontmatter::from_parts(
                    text[..open_end].to_owned(),
                    text[open_end..pos].to_owned(),
                    text[pos..pos + len].to_owned(),
                    eol,
                );
                return (fm, &text[pos + len..]);
            }
            pos = line_end;
        }
        (Frontmatter::absent(eol, true), text)
    }

    fn absent(eol: &'static str, unterminated: bool) -> Self {
        let parsed = if unterminated {
            Parsed::Error("frontmatter block is not closed with `---`".into())
        } else {
            Parsed::Map(Mapping::new())
        };
        Frontmatter {
            open: String::new(),
            raw: String::new(),
            close: String::new(),
            eol,
            parsed,
        }
    }

    fn from_parts(open: String, raw: String, close: String, eol: &'static str) -> Self {
        let parsed = parse_yaml(&raw);
        Frontmatter {
            open,
            raw,
            close,
            eol,
            parsed,
        }
    }

    /// Returns the exact source text of the block, delimiters included.
    pub fn render(&self) -> String {
        let mut s = String::with_capacity(self.open.len() + self.raw.len() + self.close.len());
        s.push_str(&self.open);
        s.push_str(&self.raw);
        s.push_str(&self.close);
        s
    }

    /// The YAML text between the delimiters.
    pub fn raw(&self) -> &str {
        &self.raw
    }

    /// The state of the block.
    pub fn state(&self) -> FrontmatterState {
        match (&self.parsed, self.open.is_empty()) {
            (Parsed::Error(_), true) => FrontmatterState::Unterminated,
            (Parsed::Error(_), false) => FrontmatterState::Invalid,
            (Parsed::Map(_), true) => FrontmatterState::Absent,
            (Parsed::Map(_), false) => FrontmatterState::Valid,
        }
    }

    /// Whether the file has a frontmatter block (valid or not).
    pub fn is_present(&self) -> bool {
        !self.open.is_empty()
    }

    /// The parse error, if the block is unterminated or invalid.
    pub fn error(&self) -> Option<&str> {
        match &self.parsed {
            Parsed::Error(e) => Some(e),
            Parsed::Map(_) => None,
        }
    }

    /// The parsed mapping, if the block is absent (empty) or valid.
    pub fn mapping(&self) -> Option<&Mapping> {
        match &self.parsed {
            Parsed::Map(m) => Some(m),
            Parsed::Error(_) => None,
        }
    }

    /// The value of a top-level key.
    pub fn get(&self, key: &str) -> Option<&Value> {
        self.mapping()?.get(key)
    }

    /// The value of a top-level key if it is a string.
    pub fn get_str(&self, key: &str) -> Option<&str> {
        self.get(key)?.as_str()
    }

    /// Top-level keys in source order.
    pub fn keys(&self) -> impl Iterator<Item = &str> {
        self.mapping()
            .into_iter()
            .flat_map(|m| m.keys().map(String::as_str))
    }

    /// Sets a top-level key, editing only the lines of that key.
    ///
    /// An existing key keeps its position and spelling; a new key is appended at
    /// the end of the block. A file without frontmatter gets a new block. Comments
    /// and formatting of other keys are preserved. Fails if the block cannot be
    /// parsed, or if its layout is too unusual to edit safely.
    pub fn set(&mut self, key: &str, value: Value) -> Result<(), Error> {
        let map = self.editable()?;
        if map.get(key) == Some(&value) {
            return Ok(());
        }
        let mut expected = map.clone();
        let raw = if map.contains_key(key) {
            let blocks = self.blocks()?;
            let block = blocks.iter().find(|b| b.key == key).expect("key present");
            let head = &self.raw[block.start..block.end];
            let flow = head[block.key_text.len()..]
                .trim_start_matches([' ', '\t', ':'])
                .starts_with('[');
            let entry = render_entry(&block.key_text, &value, flow, self.eol)?;
            expected.insert(key.to_owned(), value);
            format!(
                "{}{}{}",
                &self.raw[..block.start],
                entry,
                &self.raw[block.end..]
            )
        } else {
            let entry = render_entry(&render_key(key)?, &value, true, self.eol)?;
            expected.insert(key.to_owned(), value);
            let mut raw = self.raw.clone();
            if !raw.is_empty() && !raw.ends_with('\n') {
                raw.push_str(self.eol);
            }
            raw.push_str(&entry);
            raw
        };
        self.commit(raw, expected)
    }

    /// Removes a top-level key and its lines. Returns whether the key existed.
    pub fn remove(&mut self, key: &str) -> Result<bool, Error> {
        let map = self.editable()?;
        if !map.contains_key(key) {
            return Ok(false);
        }
        let mut expected = map.clone();
        expected.shift_remove(key);
        let blocks = self.blocks()?;
        let block = blocks.iter().find(|b| b.key == key).expect("key present");
        let raw = format!("{}{}", &self.raw[..block.start], &self.raw[block.end..]);
        self.commit(raw, expected)?;
        Ok(true)
    }

    fn editable(&self) -> Result<&Mapping, Error> {
        match &self.parsed {
            Parsed::Map(m) => Ok(m),
            Parsed::Error(e) => Err(Error::InvalidFrontmatter(e.clone())),
        }
    }

    /// Replaces the raw text after checking that it parses to `expected`, key order included.
    fn commit(&mut self, raw: String, expected: Mapping) -> Result<(), Error> {
        match parse_yaml(&raw) {
            Parsed::Map(m) if m == expected && m.keys().eq(expected.keys()) => {
                if self.open.is_empty() {
                    self.open = format!("---{}", self.eol);
                    self.close = format!("---{}", self.eol);
                }
                self.raw = raw;
                self.parsed = Parsed::Map(m);
                Ok(())
            }
            _ => Err(Error::UnsafeEdit(
                "the edit would change other keys or produce invalid YAML".into(),
            )),
        }
    }

    /// Locates the text of every top-level key. Fails unless every parsed key is found, in order.
    fn blocks(&self) -> Result<Vec<Block>, Error> {
        let raw = &self.raw;
        let mut starts: Vec<(usize, String, String)> = Vec::new();
        let mut line_starts = Vec::new();
        let mut pos = 0;
        while pos < raw.len() {
            line_starts.push(pos);
            let end = raw[pos..].find('\n').map_or(raw.len(), |i| pos + i + 1);
            let line = raw[pos..end].trim_end_matches(['\n', '\r']);
            if let Some(c) = KEY_LINE.captures(line) {
                let (key, text) = if let Some(m) = c.get(1) {
                    (unescape_double(m.as_str()), format!("\"{}\"", m.as_str()))
                } else if let Some(m) = c.get(2) {
                    (m.as_str().replace("''", "'"), format!("'{}'", m.as_str()))
                } else {
                    let m = c.get(3).expect("one branch matches");
                    (
                        m.as_str().trim_end().to_owned(),
                        m.as_str().trim_end().to_owned(),
                    )
                };
                starts.push((pos, key, text));
            }
            pos = end;
        }
        let map = self.editable()?;
        if starts.len() != map.len()
            || !starts
                .iter()
                .map(|s| s.1.as_str())
                .eq(map.keys().map(String::as_str))
        {
            return Err(Error::UnsafeEdit(
                "frontmatter layout is not supported for editing".into(),
            ));
        }
        let mut blocks = Vec::with_capacity(starts.len());
        for (i, (start, key, key_text)) in starts.iter().enumerate() {
            let limit = starts.get(i + 1).map_or(raw.len(), |s| s.0);
            // Trailing blank lines and column-0 comments belong to what follows.
            let mut end = limit;
            for &ls in line_starts
                .iter()
                .rev()
                .filter(|&&ls| ls > *start && ls < limit)
            {
                let line = raw[ls..end].trim_end_matches(['\n', '\r']);
                if line.trim().is_empty() || line.starts_with('#') {
                    end = ls;
                } else {
                    break;
                }
            }
            blocks.push(Block {
                start: *start,
                end,
                key: key.clone(),
                key_text: key_text.clone(),
            });
        }
        Ok(blocks)
    }
}

struct Block {
    start: usize,
    end: usize,
    key: String,
    key_text: String,
}

/// Length of a delimiter line (`---` with optional trailing spaces and line ending).
fn delimiter_line_len(s: &str, delim: &str) -> Option<usize> {
    let rest = s.strip_prefix(delim)?;
    let line_end = rest.find('\n').map_or(rest.len(), |i| i + 1);
    rest[..line_end]
        .trim()
        .is_empty()
        .then_some(delim.len() + line_end)
}

fn parse_yaml(raw: &str) -> Parsed {
    match serde_saphyr::from_str::<Value>(raw) {
        Ok(Value::Object(m)) => Parsed::Map(m),
        Ok(Value::Null) => Parsed::Map(Mapping::new()),
        Ok(_) => Parsed::Error("frontmatter is not a YAML mapping".into()),
        Err(e) => Parsed::Error(e.to_string()),
    }
}

fn unescape_double(s: &str) -> String {
    serde_saphyr::from_str::<String>(&format!("\"{s}\"")).unwrap_or_else(|_| s.to_owned())
}

fn yaml(value: &impl serde::Serialize) -> Result<String, Error> {
    serde_saphyr::to_string(value).map_err(|e| Error::UnsafeEdit(e.to_string()))
}

/// Plain scalars that YAML 1.1 parsers (such as PyYAML) would read as something
/// other than a string: booleans, nulls, numbers and timestamps.
static YAML11_AMBIGUOUS: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(concat!(
        r"^(?:y|Y|yes|Yes|YES|n|N|no|No|NO|true|True|TRUE|false|False|FALSE|on|On|ON|off|Off|OFF",
        r"|~|null|Null|NULL|\.(?:inf|Inf|INF|nan|NaN|NAN)|[-+]?\.(?:inf|Inf|INF)",
        r"|[-+]?[0-9][0-9_]*(?::[0-5]?[0-9])+(?:\.[0-9_]*)?",
        r"|[-+]?(?:0b[01_]+|0o?[0-7_]+|0x[0-9a-fA-F_]+|[0-9][0-9_]*(?:\.[0-9_]*)?(?:[eE][-+]?[0-9]+)?|\.[0-9_]+(?:[eE][-+]?[0-9]+)?)",
        r"|[0-9]{4}-[0-9]{1,2}-[0-9]{1,2}(?:[Tt ].*)?)$"
    ))
    .expect("valid regex")
});

/// A value that serializes ambiguous strings double-quoted, so every YAML parser reads them back as strings.
struct Safe<'a>(&'a Value);

impl serde::Serialize for Safe<'_> {
    fn serialize<S: serde::Serializer>(&self, ser: S) -> Result<S::Ok, S::Error> {
        use serde::ser::{SerializeMap, SerializeSeq};
        match self.0 {
            Value::String(s) if YAML11_AMBIGUOUS.is_match(s) => {
                serde_saphyr::DoubleQuoted(s.as_str()).serialize(ser)
            }
            Value::Array(items) => {
                let mut seq = ser.serialize_seq(Some(items.len()))?;
                for item in items {
                    seq.serialize_element(&Safe(item))?;
                }
                seq.end()
            }
            Value::Object(map) => {
                let mut m = ser.serialize_map(Some(map.len()))?;
                for (k, v) in map {
                    m.serialize_entry(&Safe(&Value::String(k.clone())), &Safe(v))?;
                }
                m.end()
            }
            other => other.serialize(ser),
        }
    }
}

fn render_key(key: &str) -> Result<String, Error> {
    if PLAIN_KEY.is_match(key) && !YAML11_AMBIGUOUS.is_match(key) {
        Ok(key.to_owned())
    } else {
        Ok(yaml(&key)?.trim_end().to_owned())
    }
}

/// Renders `key: value` as YAML lines ending with `eol`.
fn render_entry(key: &str, value: &Value, flow_seq: bool, eol: &str) -> Result<String, Error> {
    let body = match value {
        Value::Array(items) if items.is_empty() => " []\n".to_owned(),
        Value::Object(map) if map.is_empty() => " {}\n".to_owned(),
        Value::Array(items) if flow_seq && items.iter().all(is_scalar) => {
            let safe: Vec<Safe> = items.iter().map(Safe).collect();
            let flow = yaml(&serde_saphyr::FlowSeq(safe))?;
            if flow.trim_end().contains('\n') {
                indent_block(&yaml(&Safe(value))?)
            } else {
                format!(" {flow}")
            }
        }
        Value::Array(_) | Value::Object(_) => indent_block(&yaml(&Safe(value))?),
        _ => format!(" {}", yaml(&Safe(value))?),
    };
    let text = format!("{key}:{body}");
    Ok(if eol == "\n" {
        text
    } else {
        text.replace('\n', eol)
    })
}

fn indent_block(s: &str) -> String {
    let mut out = String::from("\n");
    for line in s.lines() {
        if !line.is_empty() {
            out.push_str("  ");
        }
        out.push_str(line);
        out.push('\n');
    }
    out
}

fn is_scalar(v: &Value) -> bool {
    !matches!(v, Value::Array(_) | Value::Object(_))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn fm(text: &str) -> Frontmatter {
        let (fm, body) = Frontmatter::split(text);
        assert_eq!(fm.render() + body, text);
        fm
    }

    #[test]
    fn splits_and_parses() {
        let f = fm("---\ntype: Metric # c\ntags: [a, b]\n---\n# Body\n");
        assert_eq!(f.state(), FrontmatterState::Valid);
        assert_eq!(f.get_str("type"), Some("Metric"));
        assert_eq!(f.keys().collect::<Vec<_>>(), ["type", "tags"]);
    }

    #[test]
    fn states() {
        assert_eq!(fm("# no fm\n").state(), FrontmatterState::Absent);
        assert_eq!(fm("---\ntype: A\n").state(), FrontmatterState::Unterminated);
        assert_eq!(fm("---\n- a\n---\n").state(), FrontmatterState::Invalid);
        assert_eq!(fm("---\ntype: [\n---\n").state(), FrontmatterState::Invalid);
        assert_eq!(fm("---\n---\nbody").state(), FrontmatterState::Valid);
        assert_eq!(
            fm("\u{feff}---\r\ntype: A\r\n---\r\nbody").get_str("type"),
            Some("A")
        );
        assert_eq!(fm("----\ntype: A\n---\n").state(), FrontmatterState::Absent);
    }

    #[test]
    fn set_replaces_only_target_lines() {
        let mut f = fm(
            "---\n# lead\ntype: A # keep?\ntags:\n  - x # c1\n  - y\n\n# about title\ntitle: T\n---\n",
        );
        f.set("tags", json!(["x", "z"])).unwrap();
        assert_eq!(
            f.raw(),
            "# lead\ntype: A # keep?\ntags:\n  - x\n  - z\n\n# about title\ntitle: T\n"
        );
        f.set("type", json!("B")).unwrap();
        assert_eq!(
            f.raw(),
            "# lead\ntype: B\ntags:\n  - x\n  - z\n\n# about title\ntitle: T\n"
        );
    }

    #[test]
    fn set_keeps_flow_style_and_appends_new_keys() {
        let mut f = fm("---\ntype: A\ntags: [a, b]\n---\n");
        f.set("tags", json!(["a", "b c"])).unwrap();
        f.set("status", json!("stable")).unwrap();
        f.set("generated", json!({"by": "x/1", "at": "2026-01-01"}))
            .unwrap();
        assert_eq!(
            f.raw(),
            "type: A\ntags: [a, b c]\nstatus: stable\ngenerated:\n  by: x/1\n  at: \"2026-01-01\"\n"
        );
    }

    #[test]
    fn set_quotes_ambiguous_scalars() {
        let mut f = fm("---\ntype: A\n---\n");
        for v in [
            json!("2026-01-01"),
            json!("on"),
            json!("1_000"),
            json!("12:30"),
            json!(["no", "0x1F"]),
            json!("yes"),
            json!("a: b"),
            json!("2026"),
            json!("multi\nline"),
            json!(""),
            json!(null),
        ] {
            f.set("k", v.clone()).unwrap();
            assert_eq!(f.get("k"), Some(&v));
        }
    }

    #[test]
    fn ambiguous_strings_are_double_quoted() {
        let mut f = fm("---\ntype: A\n---\n");
        f.set("at", json!("2026-01-01")).unwrap();
        f.set("flags", json!(["on", "x", "1.5"])).unwrap();
        f.set("yes", json!(1)).unwrap();
        assert_eq!(
            f.raw(),
            "type: A\nat: \"2026-01-01\"\nflags: [\"on\", x, \"1.5\"]\n\"yes\": 1\n"
        );
    }

    #[test]
    fn set_on_absent_creates_block() {
        let text = "# Title\n";
        let (mut f, body) = Frontmatter::split(text);
        f.set("type", json!("Guide")).unwrap();
        assert_eq!(f.render() + body, "---\ntype: Guide\n---\n# Title\n");
    }

    #[test]
    fn set_same_value_is_noop() {
        let mut f = fm("---\ntype:   A   # odd spacing\n---\n");
        let before = f.clone();
        f.set("type", json!("A")).unwrap();
        assert_eq!(f, before);
    }

    #[test]
    fn remove_key_and_crlf() {
        let mut f = fm(
            "---\r\ntype: A\r\ndescription: >-\r\n  folded\r\n  text\r\n# c\r\ntitle: T\r\n---\r\n",
        );
        assert!(f.remove("description").unwrap());
        assert!(!f.remove("description").unwrap());
        assert_eq!(f.raw(), "type: A\r\n# c\r\ntitle: T\r\n");
        f.set("tags", json!(["a"])).unwrap();
        assert_eq!(f.raw(), "type: A\r\n# c\r\ntitle: T\r\ntags: [a]\r\n");
    }

    #[test]
    fn quoted_and_odd_keys() {
        let mut f = fm("---\n\"odd key\": 1\n'it''s': 2\nurl:x: 3\ntype: A\n---\n");
        assert_eq!(
            f.keys().collect::<Vec<_>>(),
            ["odd key", "it's", "url:x", "type"]
        );
        f.set("it's", json!(5)).unwrap();
        assert_eq!(f.raw(), "\"odd key\": 1\n'it''s': 5\nurl:x: 3\ntype: A\n");
        f.set("new key", json!(true)).unwrap();
        assert_eq!(f.get("new key"), Some(&json!(true)));
    }

    #[test]
    fn refuses_unsupported_layouts() {
        let mut f = fm("---\n{type: A, title: T}\n---\n");
        assert!(matches!(
            f.set("type", json!("B")),
            Err(Error::UnsafeEdit(_))
        ));
        let mut f = fm("---\ntype: [\n---\n");
        assert!(matches!(
            f.set("type", json!("B")),
            Err(Error::InvalidFrontmatter(_))
        ));
    }
}
