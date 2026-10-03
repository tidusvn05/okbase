use std::sync::LazyLock;

use rust_stemmers::{Algorithm, Stemmer};

use crate::fold;

/// Whether `c` is kana, a CJK ideograph or half-width katakana.
pub fn is_cjk(c: char) -> bool {
    matches!(c as u32,
        0x3040..=0x30FF | 0x3400..=0x4DBF | 0x4E00..=0x9FFF | 0xF900..=0xFAFF | 0xFF66..=0xFF9F)
}

static STEMMER: LazyLock<Stemmer> = LazyLock::new(|| Stemmer::create(Algorithm::English));

/// Turns text into terms for the full-text index. Use the same analyzer for
/// documents and queries.
///
/// Latin-script runs are split on non-alphanumeric characters, [`fold`]ed and
/// (optionally) stemmed with the English Snowball stemmer. CJK runs are
/// segmented with lindera (feature `ja`): particles, auxiliary verbs and symbols
/// are dropped and words are reduced to their dictionary form (`食べた` → `食べる`).
#[derive(Debug, Clone)]
pub struct Analyzer {
    stem: bool,
}

impl Default for Analyzer {
    fn default() -> Self {
        Analyzer { stem: true }
    }
}

impl Analyzer {
    /// An analyzer with English stemming on.
    pub fn new() -> Self {
        Self::default()
    }

    /// Turns English stemming on or off.
    pub fn with_stemming(mut self, stem: bool) -> Self {
        self.stem = stem;
        self
    }

    /// The terms of `text`, in order.
    pub fn terms(&self, text: &str) -> Vec<String> {
        let text: String = unicode_normalization::UnicodeNormalization::nfkc(text).collect();
        let mut out = Vec::new();
        let mut start = 0;
        let mut run_cjk = false;
        for (i, c) in text.char_indices() {
            let cjk = is_cjk(c);
            if cjk != run_cjk && i > start {
                self.push_run(&text[start..i], run_cjk, &mut out);
                start = i;
            }
            run_cjk = cjk;
        }
        self.push_run(&text[start..], run_cjk, &mut out);
        out
    }

    /// The terms of `text` joined by single spaces, ready for an FTS5 column or query.
    pub fn fts_text(&self, text: &str) -> String {
        self.terms(text).join(" ")
    }

    fn push_run(&self, run: &str, cjk: bool, out: &mut Vec<String>) {
        if run.is_empty() {
            return;
        }
        if cjk {
            cjk_terms(run, out);
            return;
        }
        for word in run
            .split(|c: char| !c.is_alphanumeric())
            .filter(|w| !w.is_empty())
        {
            let word = fold(word);
            if self.stem && word.is_ascii() && word.chars().any(|c| c.is_ascii_alphabetic()) {
                out.push(STEMMER.stem(&word).into_owned());
            } else {
                out.push(word);
            }
        }
    }
}

#[cfg(feature = "ja")]
mod ja {
    use std::borrow::Cow;
    use std::sync::OnceLock;

    use lindera::mode::Mode;
    use lindera::segmenter::Segmenter;

    /// The IPADIC segmenter, loaded on first use: embedded (feature `ja-embedded`),
    /// else from the user cache, else downloaded (feature `ja-download`). `None`
    /// when unavailable; callers fall back to bigrams.
    static SEGMENTER: OnceLock<Option<Segmenter>> = OnceLock::new();

    fn load() -> Option<Segmenter> {
        #[cfg(feature = "ja-embedded")]
        if let Ok(dict) = lindera::dictionary::load_dictionary("embedded://ipadic") {
            return Some(Segmenter::new(Mode::Normal, dict, None));
        }
        let dir = match crate::dict::install() {
            Ok(dir) => dir,
            Err(e) => {
                eprintln!("okbase: {e}; Japanese text is indexed as character bigrams");
                return None;
            }
        };
        match lindera::dictionary::load_fs_dictionary(&dir) {
            Ok(dict) => Some(Segmenter::new(Mode::Normal, dict, None)),
            Err(e) => {
                eprintln!(
                    "okbase: cannot load the Japanese dictionary from {}: {e}",
                    dir.display()
                );
                None
            }
        }
    }

    /// Whether dictionary-based segmentation is (or will be, without a download) available.
    pub fn ready() -> bool {
        SEGMENTER.get().map_or_else(
            || cfg!(feature = "ja-embedded") || crate::dict::installed(),
            Option::is_some,
        )
    }

    /// Segments a CJK run into dictionary forms. Returns `false` if segmentation is unavailable.
    pub fn terms(run: &str, out: &mut Vec<String>) -> bool {
        let Some(seg) = SEGMENTER.get_or_init(load).as_ref() else {
            return false;
        };
        let Ok(tokens) = seg.segment(Cow::Borrowed(run)) else {
            return false;
        };
        for mut t in tokens {
            let surface = t.surface.to_string();
            let details = t.details();
            if matches!(details.first().copied(), Some("助詞" | "助動詞" | "記号")) {
                continue;
            }
            let base = details
                .get(6)
                .filter(|b| **b != "*")
                .map_or(surface, |b| (*b).to_owned());
            out.push(crate::fold(&base));
        }
        true
    }
}

/// How Japanese/Chinese text is tokenized right now: `ipadic` (dictionary forms)
/// or `bigram`. Part of the index's analyzer identity, so a newly installed
/// dictionary triggers a rebuild.
pub fn cjk_mode() -> &'static str {
    #[cfg(feature = "ja")]
    if ja::ready() {
        return "ipadic";
    }
    "bigram"
}

fn cjk_terms(run: &str, out: &mut Vec<String>) {
    #[cfg(feature = "ja")]
    if ja::terms(run, out) {
        return;
    }
    bigrams(run, out);
}

/// Overlapping character bigrams (a single character stays a unigram).
fn bigrams(run: &str, out: &mut Vec<String>) {
    let chars: Vec<char> = fold(run).chars().collect();
    if chars.len() == 1 {
        out.push(chars[0].to_string());
    }
    out.extend(chars.windows(2).map(|w| w.iter().collect()));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn latin_terms() {
        let a = Analyzer::new();
        assert_eq!(
            a.terms("Đổi trả hàng: dm_scope=ON"),
            ["doi", "tra", "hang", "dm", "scope", "on"]
        );
        assert_eq!(a.terms("Running refunds"), ["run", "refund"]);
        assert_eq!(
            Analyzer::new()
                .with_stemming(false)
                .terms("Running refunds"),
            ["running", "refunds"]
        );
        assert_eq!(a.fts_text("ＡＢＣ 123"), "abc 123");
    }

    #[cfg(feature = "ja")]
    #[test]
    fn japanese_dictionary_forms() {
        let a = Analyzer::new();
        if a.terms("食べた") == ["食べ", "べた"] {
            // No dictionary (offline, OKBASE_OFFLINE=1): bigram fallback.
            assert_eq!(crate::cjk_mode(), "bigram");
            return;
        }
        assert_eq!(crate::cjk_mode(), "ipadic");
        assert_eq!(a.terms("食べた"), ["食べる"]);
        let t = a.terms("パスワードを変更しました。");
        assert!(
            t.contains(&"パスワード".to_owned()) && t.contains(&"変更".to_owned()),
            "{t:?}"
        );
        assert!(!t.contains(&"を".to_owned()));
        assert_eq!(
            a.terms("API キーを設定する"),
            ["api", "キー", "設定", "する"]
        );
    }

    #[test]
    fn bigram_fallback() {
        let mut out = Vec::new();
        bigrams("東京都", &mut out);
        assert_eq!(out, ["東京", "京都"]);
    }
}
