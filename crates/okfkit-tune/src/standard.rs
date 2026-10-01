//! The okfkit question standard v1 (docs/PLAN-advise-tune.md §3): what an agent writes for each
//! passage, and the checks every question must pass.

use std::collections::{BTreeMap, HashSet};

use serde::{Deserialize, Serialize};

/// Standard id, recorded in runs and model provenance.
pub const STANDARD: &str = "okfkit-questions/v1";

/// Kinds of questions.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Kind {
    /// A natural question in the passage's language.
    Natural,
    /// An agent-style keyword query.
    Keyword,
    /// A question in another language.
    Cross,
    /// Vague, paraphrased, possibly with typos.
    Vague,
}

impl Kind {
    /// All kinds.
    pub const ALL: [Kind; 4] = [Kind::Natural, Kind::Keyword, Kind::Cross, Kind::Vague];

    /// Name as written in JSONL.
    pub fn name(self) -> &'static str {
        match self {
            Kind::Natural => "natural",
            Kind::Keyword => "keyword",
            Kind::Cross => "cross",
            Kind::Vague => "vague",
        }
    }
}

/// Settings of a run (defaults = standard v1).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Settings {
    /// Languages people ask in (ISO 639-1).
    pub langs: Vec<String>,
    /// At most this many passages are sampled.
    pub max_passages: usize,
    /// Share of documents held out for evaluation.
    pub heldout_share: f64,
    /// At least this many held-out documents (when the bundle allows).
    pub min_heldout_docs: usize,
    /// Passages per batch.
    pub batch_passages: usize,
    /// Estimated tokens of passage text per batch.
    pub batch_tokens: usize,
    /// Chunks shorter than this are skipped.
    pub min_chunk_tokens: usize,
    /// Questions for a passage that is a whole document.
    pub per_document: usize,
    /// Questions for a chunk of a longer document.
    pub per_chunk: usize,
    /// Training pairs needed before training is allowed.
    pub min_pairs: usize,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            langs: vec!["en".into()],
            max_passages: 400,
            heldout_share: 0.15,
            min_heldout_docs: 20,
            batch_passages: 10,
            batch_tokens: 6_000,
            min_chunk_tokens: 40,
            per_document: 4,
            per_chunk: 2,
            min_pairs: 300,
        }
    }
}

/// A passage to write questions for: one chunk of the index.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Passage {
    /// `<doc id>#<chunk ord>`.
    pub key: String,
    /// Document id.
    pub doc: String,
    /// Document title.
    pub title: String,
    /// Section heading (`A > B`).
    pub heading: String,
    /// Chunk text.
    pub text: String,
    /// Detected language (`vi`, `en`, `ja` or `other`).
    pub lang: String,
    /// Estimated tokens.
    pub tokens: usize,
    /// The chunk is the whole document.
    pub whole: bool,
    /// The document is held out for evaluation.
    pub heldout: bool,
}

impl Passage {
    /// Questions required for this passage.
    pub fn quota(&self, s: &Settings) -> usize {
        if self.whole {
            s.per_document
        } else {
            s.per_chunk
        }
    }

    /// Whether a cross-language question is possible.
    pub fn cross_possible(&self, langs: &[String]) -> bool {
        langs.iter().any(|l| *l != self.lang)
    }
}

/// A question as the agent writes it (one JSONL line).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Question {
    /// Passage key.
    pub passage: String,
    /// Kind.
    pub kind: Kind,
    /// Language of the question.
    pub lang: String,
    /// The question.
    pub q: String,
}

/// Normalized form for duplicate detection.
pub fn normalize(q: &str) -> String {
    okfkit_analyze::fold(q)
        .chars()
        .map(|c| if c.is_alphanumeric() { c } else { ' ' })
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

fn has_cjk(s: &str) -> bool {
    s.chars().any(okfkit_analyze::is_cjk)
}

/// Words, or characters for CJK text.
fn length(q: &str) -> usize {
    if has_cjk(q) {
        q.chars().filter(|c| !c.is_whitespace()).count()
    } else {
        q.split_whitespace().count()
    }
}

fn length_bounds(kind: Kind, lang: &str) -> (usize, usize) {
    match (kind, lang) {
        (Kind::Keyword, "ja") => (2, 25),
        // Vietnamese words are counted by syllable: "bảo hành máy giặt" is 4.
        (Kind::Keyword, "vi") => (2, 12),
        (Kind::Keyword, _) => (2, 8),
        (_, "ja") => (5, 60),
        (_, "vi") => (3, 40),
        _ => (3, 30),
    }
}

/// Checks the script of a question against its declared language. Lenient where detection is
/// unreliable (short Latin-script text), strict where the script decides. Vague Vietnamese
/// questions may drop diacritics, as people often type them.
fn lang_matches(q: &str, lang: &str, kind: Kind) -> bool {
    let vi_letters =
        okfkit_analyze::detect_lang(q) == Some(okfkit_analyze::Lang::Vi) && !has_cjk(q);
    match lang {
        "ja" => has_cjk(q),
        "vi" if kind == Kind::Vague => !has_cjk(q),
        "vi" => vi_letters,
        "en" => !has_cjk(q) && !vi_letters,
        _ => true,
    }
}

fn ngrams(words: &[&str], n: usize) -> HashSet<String> {
    words.windows(n).map(|w| w.join(" ")).collect()
}

/// Copying checks against a passage: 5 consecutive words (8 syllables for Vietnamese,
/// 12 characters for CJK).
pub(crate) struct CopyIndex {
    run: usize,
    words: HashSet<String>,
    chars12: HashSet<String>,
    title: String,
}

impl CopyIndex {
    pub(crate) fn new(p: &Passage) -> Self {
        let text = normalize(&p.text);
        let words: Vec<&str> = text.split(' ').collect();
        let chars: Vec<char> = text.chars().filter(|c| !c.is_whitespace()).collect();
        let chars12 = if has_cjk(&p.text) {
            chars.windows(12).map(|w| w.iter().collect()).collect()
        } else {
            HashSet::new()
        };
        let title = normalize(&p.title);
        let vi = p.lang == "vi";
        let run = if vi { 8 } else { 5 };
        CopyIndex {
            run,
            words: ngrams(&words, run),
            chars12,
            // Short titles are the topic itself; only longer ones count as copying.
            title: if title.split(' ').count() >= if vi { 6 } else { 3 }
                || (has_cjk(&p.title) && title.chars().count() >= 8)
            {
                title
            } else {
                String::new()
            },
        }
    }

    /// Keyword queries may name the topic (title); other kinds must not repeat it.
    fn copies(&self, q: &str, kind: Kind) -> Option<String> {
        let n = normalize(q);
        if kind != Kind::Keyword && !self.title.is_empty() && n.contains(&self.title) {
            return Some("repeats the title".into());
        }
        let words: Vec<&str> = n.split(' ').collect();
        if ngrams(&words, self.run)
            .iter()
            .any(|g| self.words.contains(g))
        {
            return Some(format!(
                "copies {}+ consecutive {} from the passage",
                self.run,
                if self.run == 5 { "words" } else { "syllables" }
            ));
        }
        if !self.chars12.is_empty() {
            let chars: Vec<char> = n.chars().filter(|c| !c.is_whitespace()).collect();
            if chars
                .windows(12)
                .any(|w| self.chars12.contains(&w.iter().collect::<String>()))
            {
                return Some("copies 12+ consecutive characters from the passage".into());
            }
        }
        None
    }
}

/// Checks one batch of questions. Returns the errors (empty = accepted), each prefixed with
/// the 1-based line number when it concerns a line.
pub fn check_batch(
    passages: &[Passage],
    settings: &Settings,
    jsonl: &str,
    seen: &HashSet<String>,
) -> (Vec<Question>, Vec<String>) {
    let mut errors = Vec::new();
    let mut out = Vec::new();
    let by_key: BTreeMap<&str, (&Passage, CopyIndex)> = passages
        .iter()
        .map(|p| (p.key.as_str(), (p, CopyIndex::new(p))))
        .collect();
    let mut batch_seen = HashSet::new();
    let mut per: BTreeMap<&str, Vec<Kind>> = BTreeMap::new();
    for (i, line) in jsonl.lines().enumerate() {
        let line = line.trim();
        if line.is_empty() || line.starts_with("```") {
            continue;
        }
        let n = i + 1;
        let q: Question = match serde_json::from_str(line) {
            Ok(q) => q,
            Err(e) => {
                errors.push(format!(
                    "line {n}: not a question object ({e}); expected {{\"passage\", \"kind\", \"lang\", \"q\"}}"
                ));
                continue;
            }
        };
        let Some((p, copy)) = by_key.get(q.passage.as_str()) else {
            errors.push(format!(
                "line {n}: unknown passage `{}` (not in this batch)",
                q.passage
            ));
            continue;
        };
        let lang = q.lang.to_lowercase();
        let mut line_errors = Vec::new();
        let cross_possible = p.cross_possible(&settings.langs);
        match q.kind {
            Kind::Natural if p.lang != "other" && lang != p.lang => line_errors.push(format!(
                "natural questions use the passage's language ({}), not {lang}",
                p.lang
            )),
            Kind::Cross if !cross_possible => line_errors.push(
                "no cross question here: the passage is already in every listed language".into(),
            ),
            Kind::Cross if lang == p.lang || !settings.langs.contains(&lang) => {
                line_errors.push(format!(
                    "cross questions use another listed language ({}), not {lang}",
                    settings
                        .langs
                        .iter()
                        .filter(|l| **l != p.lang)
                        .cloned()
                        .collect::<Vec<_>>()
                        .join("/")
                ))
            }
            Kind::Keyword | Kind::Vague if lang != p.lang && !settings.langs.contains(&lang) => {
                line_errors.push(format!(
                    "language {lang} is not listed ({}) and is not the passage's",
                    settings.langs.join("/")
                ))
            }
            _ => {}
        }
        if !lang_matches(&q.q, &lang, q.kind) {
            line_errors.push(format!("the text does not look like `{lang}`"));
        }
        let (lo, hi) = length_bounds(q.kind, &lang);
        let len = length(&q.q);
        if len < lo || len > hi {
            let unit = if has_cjk(&q.q) { "characters" } else { "words" };
            line_errors.push(format!("{len} {unit}; {} needs {lo}–{hi}", q.kind.name()));
        }
        if let Some(why) = copy.copies(&q.q, q.kind) {
            line_errors.push(why);
        }
        let lower = q.q.to_lowercase();
        if lower.contains(".md") || (p.doc.contains('/') && lower.contains(&p.doc.to_lowercase())) {
            line_errors.push("mentions a file name or document id".into());
        }
        let norm = normalize(&q.q);
        if seen.contains(&norm) || !batch_seen.insert(norm) {
            line_errors.push("duplicate of another question".into());
        }
        if line_errors.is_empty() {
            per.entry(by_key.get_key_value(q.passage.as_str()).unwrap().0)
                .or_default()
                .push(q.kind);
            out.push(Question { lang, ..q });
        } else {
            errors.push(format!(
                "line {n} ({}): {}",
                q.passage,
                line_errors.join("; ")
            ));
        }
    }
    // Quotas and kinds per passage.
    for p in passages {
        let kinds = per.get(p.key.as_str()).cloned().unwrap_or_default();
        let need = p.quota(settings);
        if let Some(e) = quota_error(p, &kinds, need, p.cross_possible(&settings.langs)) {
            errors.push(format!("passage {}: {e}", p.key));
        }
    }
    (out, errors)
}

fn quota_error(p: &Passage, kinds: &[Kind], need: usize, cross: bool) -> Option<String> {
    let _ = p;
    if kinds.len() != need {
        return Some(format!(
            "has {} valid questions, needs exactly {need}",
            kinds.len()
        ));
    }
    let count = |k: Kind| kinds.iter().filter(|x| **x == k).count();
    if need >= 4 {
        let required: &[Kind] = if cross {
            &Kind::ALL
        } else {
            &[Kind::Natural, Kind::Keyword, Kind::Vague]
        };
        let missing: Vec<&str> = required
            .iter()
            .filter(|k| count(**k) == 0)
            .map(|k| k.name())
            .collect();
        if !missing.is_empty() {
            return Some(format!("missing kinds: {}", missing.join(", ")));
        }
    } else if cross && count(Kind::Cross) != 1 {
        return Some("needs exactly one cross question".into());
    } else if !cross && kinds.windows(2).any(|w| w[0] == w[1]) {
        return Some("needs two different kinds".into());
    }
    None
}

/// The instructions printed with each batch (kept short: agents read it once per batch).
pub fn prompt(settings: &Settings) -> String {
    let langs = settings.langs.join(", ");
    format!(
        "Write search queries that people or AI agents would use to find each passage below. Languages: {langs}.\n\
         - Passage marked [{d}]: one of each kind: natural (in the passage's language), keyword (2-8 words, may mix\n\
         \x20 English terms; Vietnamese 2-12 syllables, Japanese 2-25 characters), cross (in another listed language),\n\
         \x20 vague (paraphrased or vague; typos and Vietnamese without diacritics are fine).\n\
         - Passage marked [{c}]: one cross plus one natural, keyword or vague.\n\
         - If a passage is already in every listed language, skip cross and use the other kinds.\n\
         - Each query must be answerable from that passage alone. Do not copy 5+ consecutive words (Vietnamese: 8\n\
         \x20 syllables), do not repeat a long title except in keyword queries, and do not mention file names or ids.\n\
         \x20 Natural/cross/vague: 3-30 words (Vietnamese: 3-40 syllables, Japanese: 5-60 characters).\n\
         Output JSONL only, one object per line:\n\
         {{\"passage\": \"<key>\", \"kind\": \"natural|keyword|cross|vague\", \"lang\": \"<code>\", \"q\": \"...\"}}\n",
        d = settings.per_document,
        c = settings.per_chunk,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn passage(key: &str, lang: &str, whole: bool, text: &str) -> Passage {
        Passage {
            key: key.into(),
            doc: key.split('#').next().unwrap().into(),
            title: "Annual paid leave in Japan".into(),
            heading: String::new(),
            text: text.into(),
            lang: lang.into(),
            tokens: 100,
            whole,
            heldout: false,
        }
    }

    fn settings() -> Settings {
        Settings {
            langs: vec!["vi".into(), "ja".into(), "en".into()],
            ..Default::default()
        }
    }

    const TEXT: &str = "Employees at the Tokyo and Osaka offices receive 10 days of paid annual leave once they have worked for six months.";

    #[test]
    fn accepts_a_complete_batch() {
        let ps = [
            passage("hr/leave#0", "en", true, TEXT),
            passage(
                "hr/pay#1",
                "en",
                false,
                "Salaries are paid on the 25th of each month by bank transfer.",
            ),
        ];
        let jsonl = r#"
{"passage": "hr/leave#0", "kind": "natural", "lang": "en", "q": "How many vacation days do new staff in Japan get?"}
{"passage": "hr/leave#0", "kind": "keyword", "lang": "en", "q": "paid leave days Tokyo"}
{"passage": "hr/leave#0", "kind": "cross", "lang": "vi", "q": "Nhân viên ở Nhật được nghỉ phép bao nhiêu ngày?"}
{"passage": "hr/leave#0", "kind": "vague", "lang": "ja", "q": "日本の休みは何日もらえる？"}
{"passage": "hr/pay#1", "kind": "cross", "lang": "ja", "q": "給料はいつ振り込まれますか"}
{"passage": "hr/pay#1", "kind": "keyword", "lang": "en", "q": "salary payday"}
"#;
        let (qs, errors) = check_batch(&ps, &settings(), jsonl, &HashSet::new());
        assert!(errors.is_empty(), "{errors:#?}");
        assert_eq!(qs.len(), 6);
    }

    #[test]
    fn rejects_with_reasons() {
        let ps = [passage("hr/leave#0", "en", false, TEXT)];
        let jsonl = r#"
{"passage": "hr/leave#0", "kind": "natural", "lang": "vi", "q": "Nhân viên được nghỉ bao nhiêu ngày?"}
{"passage": "hr/leave#0", "kind": "cross", "lang": "ja", "q": "how many days of leave"}
{"passage": "hr/leave#0", "kind": "keyword", "lang": "en", "q": "receive 10 days of paid annual leave"}
{"passage": "hr/other#0", "kind": "keyword", "lang": "en", "q": "x y"}
not json
{"passage": "hr/leave#0", "kind": "vague", "lang": "en", "q": "what does hr/leave.md say about Annual paid leave in Japan"}
{"passage": "hr/leave#0", "kind": "keyword", "lang": "en", "q": "leave policy for staff in all of the japanese offices please"}
"#;
        let (_, errors) = check_batch(&ps, &settings(), jsonl, &HashSet::new());
        let all = errors.join("\n");
        for needle in [
            "natural questions use the passage's language (en)",
            "does not look like `ja`",
            "copies 5+ consecutive words",
            "unknown passage `hr/other#0`",
            "line 6: not a question object",
            "repeats the title",
            "mentions a file name",
            "keyword needs 2–8",
            "has 0 valid questions, needs exactly 2",
        ] {
            assert!(all.contains(needle), "missing `{needle}` in:\n{all}");
        }
    }

    #[test]
    fn duplicates_and_single_language() {
        let s = Settings::default(); // en only: no cross questions
        let ps = [passage("a#0", "en", false, TEXT)];
        let seen: HashSet<String> = [normalize("Salary payday?")].into();
        let jsonl = r#"
{"passage": "a#0", "kind": "keyword", "lang": "en", "q": "salary  PAYDAY"}
{"passage": "a#0", "kind": "natural", "lang": "en", "q": "When do people get their leave days?"}
"#;
        let (_, errors) = check_batch(&ps, &s, jsonl, &seen);
        assert!(errors.iter().any(|e| e.contains("duplicate")), "{errors:?}");
        let jsonl = r#"
{"passage": "a#0", "kind": "cross", "lang": "vi", "q": "Nhân viên được nghỉ bao nhiêu ngày?"}
{"passage": "a#0", "kind": "natural", "lang": "en", "q": "When do people get their leave days?"}
"#;
        let (_, errors) = check_batch(&ps, &s, jsonl, &HashSet::new());
        assert!(
            errors.iter().any(|e| e.contains("no cross question here")),
            "{errors:?}"
        );
    }

    #[test]
    fn vietnamese_counts_syllables() {
        let mut p = passage(
            "kb/vi-washer#0",
            "vi",
            false,
            "Máy giặt báo lỗi khi lồng giặt mất cân bằng, hãy dàn đều quần áo rồi khởi động lại máy.",
        );
        p.title = "Bảo hành máy giặt".into();
        let ok = r#"
{"passage": "kb/vi-washer#0", "kind": "keyword", "lang": "vi", "q": "bảo hành máy giặt lỗi lồng giặt mất cân bằng"}
{"passage": "kb/vi-washer#0", "kind": "cross", "lang": "en", "q": "washer shows an unbalanced drum error, what now?"}
"#;
        let (_, errors) = check_batch(&[p.clone()], &settings(), ok, &HashSet::new());
        assert!(errors.is_empty(), "{errors:#?}");
        let bad = r#"
{"passage": "kb/vi-washer#0", "kind": "natural", "lang": "vi", "q": "lồng giặt mất cân bằng, hãy dàn đều quần áo rồi làm gì?"}
{"passage": "kb/vi-washer#0", "kind": "cross", "lang": "en", "q": "washer shows an unbalanced drum error, what now?"}
"#;
        let (_, errors) = check_batch(&[p.clone()], &settings(), bad, &HashSet::new());
        assert!(
            errors
                .iter()
                .any(|e| e.contains("8+ consecutive syllables")),
            "{errors:#?}"
        );
        // Vague questions may drop diacritics, as people type them.
        let unaccented = r#"
{"passage": "kb/vi-washer#0", "kind": "vague", "lang": "vi", "q": "may giat keu loi ko quay duoc"}
{"passage": "kb/vi-washer#0", "kind": "cross", "lang": "ja", "q": "洗濯機のバランスエラーの直し方"}
"#;
        let (qs, errors) = check_batch(&[p], &settings(), unaccented, &HashSet::new());
        assert!(errors.is_empty() && qs.len() == 2, "{errors:#?}");
    }

    #[test]
    fn prompt_snapshot() {
        let p = prompt(&settings());
        assert!(p.contains("Languages: vi, ja, en."));
        assert!(p.contains("Passage marked [4]") && p.contains("Passage marked [2]"));
    }
}
