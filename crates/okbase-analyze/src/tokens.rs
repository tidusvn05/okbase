//! Approximate token counts without a tokenizer model.
//!
//! **These are estimates.** A linear model over simple counts, fitted on 2,752
//! chunks (OpenClaw docs, the multilingual and business fixtures; vi/en/ja,
//! prose, tables, code) against the EmbeddingGemma tokenizer: median error +1%,
//! 9 in 10 chunks within −7%…+10%. It replaces the spike's per-word rates,
//! which ignored punctuation and digits and undercounted code-heavy text by
//! ~30% (enough to push chunks past the 512-token model limit).

use crate::is_cjk;

/// Tokens per Latin-script word without Vietnamese letters.
const PER_WORD: f64 = 1.09;
/// Tokens per word containing Vietnamese (or other accented Latin) letters.
const PER_VI_SYLLABLE: f64 = 1.01;
/// Tokens per kana/ideograph.
const PER_CJK: f64 = 0.51;
/// Tokens per punctuation or symbol character.
const PER_PUNCT: f64 = 0.63;
/// Tokens per digit (numbers are split into digits).
const PER_DIGIT: f64 = 1.40;
/// Tokens per line break (indentation and markup that follows it).
const PER_NEWLINE: f64 = 1.64;

/// Estimates the number of LLM tokens in `text`.
pub fn estimate_tokens(text: &str) -> usize {
    let mut total = 0.0;
    let (mut letters, mut vi) = (false, false);
    let end = |letters: &mut bool, vi: &mut bool| -> f64 {
        let t = match (*letters, *vi) {
            (false, _) => 0.0,
            (true, false) => PER_WORD,
            (true, true) => PER_VI_SYLLABLE,
        };
        *letters = false;
        *vi = false;
        t
    };
    for c in text.chars() {
        if is_cjk(c) {
            total += PER_CJK + end(&mut letters, &mut vi);
        } else if c.is_ascii_digit() {
            total += PER_DIGIT;
        } else if c.is_alphanumeric() {
            letters = true;
            vi |= !c.is_ascii() && ('\u{00c0}'..='\u{1ef9}').contains(&c);
        } else {
            total += end(&mut letters, &mut vi);
            if c == '\n' {
                total += PER_NEWLINE;
            } else if !c.is_whitespace() {
                total += PER_PUNCT;
            }
        }
    }
    total += end(&mut letters, &mut vi);
    total.ceil() as usize
}

#[cfg(test)]
mod tests {
    use super::estimate_tokens;

    #[test]
    fn rates() {
        assert_eq!(estimate_tokens(""), 0);
        assert_eq!(estimate_tokens("one two three four five"), 6); // 5 × 1.09 = 5.45
        assert_eq!(estimate_tokens("chính sách đổi trả"), 5); // 4 × 1.01 = 4.04
        assert_eq!(estimate_tokens("返品ポリシー"), 4); // 6 × 0.51 = 3.06
        // Code and numbers cost more than their words suggest: 1 word + 5 digits + 6 symbols.
        assert_eq!(estimate_tokens("{\"port\": 18789}"), 12);
    }
}
