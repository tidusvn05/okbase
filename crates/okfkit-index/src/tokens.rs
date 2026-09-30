//! Approximate token counts without a tokenizer model.
//!
//! **These are estimates.** The rates come from the embed-bench spike, measured
//! with the Gemma tokenizer: English ≈ 1.24 tokens per word, Vietnamese ≈ 1.28
//! tokens per syllable, Japanese ≈ 0.53 tokens per character. Code and markup
//! may deviate noticeably.

use okfkit_analyze::is_cjk;

const EN_PER_WORD: f64 = 1.24;
const VI_PER_SYLLABLE: f64 = 1.28;
const JA_PER_CHAR: f64 = 0.53;

/// Estimates the number of LLM tokens in `text`.
pub fn estimate_tokens(text: &str) -> usize {
    let mut total = 0.0;
    let mut word = false;
    let mut vi = false;
    for c in text.chars() {
        if is_cjk(c) {
            total += JA_PER_CHAR + end_word(&mut word, &mut vi);
        } else if c.is_alphanumeric() {
            word = true;
            vi |= is_vietnamese(c);
        } else {
            total += end_word(&mut word, &mut vi);
        }
    }
    total += end_word(&mut word, &mut vi);
    total.ceil() as usize
}

fn end_word(word: &mut bool, vi: &mut bool) -> f64 {
    let t = match (*word, *vi) {
        (false, _) => 0.0,
        (true, false) => EN_PER_WORD,
        (true, true) => VI_PER_SYLLABLE,
    };
    *word = false;
    *vi = false;
    t
}

fn is_vietnamese(c: char) -> bool {
    !c.is_ascii() && ('\u{00c0}'..='\u{1ef9}').contains(&c)
}

#[cfg(test)]
mod tests {
    use super::estimate_tokens;

    #[test]
    fn rates() {
        assert_eq!(estimate_tokens(""), 0);
        assert_eq!(estimate_tokens("one two three four five"), 7); // 5 × 1.24 = 6.2
        assert_eq!(estimate_tokens("chính sách đổi trả"), 6); // 4 × 1.28 = 5.12
        assert_eq!(estimate_tokens("返品ポリシー"), 4); // 6 × 0.53 = 3.18
    }
}
