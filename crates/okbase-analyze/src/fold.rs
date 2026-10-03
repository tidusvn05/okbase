use unicode_normalization::UnicodeNormalization;
use unicode_normalization::char::is_combining_mark;

/// Folds text for accent-, width- and case-insensitive matching.
///
/// Applies NFKC (full-width `ＡＢＣ` → `ABC`), removes Latin combining accents
/// (`đổi trả` → `doi tra`), maps `đ`/`Đ` to `d`, lowercases, and recomposes
/// with NFC so Japanese voiced kana (`が`) stay intact.
///
/// ```
/// assert_eq!(okbase_analyze::fold("Đổi trả ＡＢＣ"), "doi tra abc");
/// assert_eq!(okbase_analyze::fold("ガイド"), "ガイド");
/// ```
pub fn fold(s: &str) -> String {
    if s.is_ascii() {
        return s.to_ascii_lowercase(); // NFKC and accent removal are identities on ASCII
    }
    let folded: String = s
        .nfkc()
        .collect::<String>()
        .nfd()
        .filter(|&c| !is_latin_accent(c))
        .map(|c| if c == 'đ' || c == 'Đ' { 'd' } else { c })
        .collect::<String>()
        .to_lowercase();
    folded.nfc().collect()
}

/// Combining marks used by Latin scripts (U+0300–U+036F), including all Vietnamese tone marks.
/// Other combining marks (such as the Japanese voicing marks U+3099/U+309A) are kept.
fn is_latin_accent(c: char) -> bool {
    ('\u{0300}'..='\u{036f}').contains(&c) && is_combining_mark(c)
}

#[cfg(test)]
mod tests {
    use super::fold;

    #[test]
    fn spike_examples() {
        assert_eq!(fold("đổi trả"), fold("doi tra"));
        assert_eq!(fold("ＡＢＣ"), "abc");
        assert_eq!(fold("Hoàn Tiền"), "hoan tien");
        assert_eq!(fold("ĐƯỜNG"), "duong");
        assert_eq!(fold("Crème brûlée"), "creme brulee");
    }

    #[test]
    fn japanese_kept() {
        assert_eq!(fold("ｶﾞｲﾄﾞ"), "ガイド");
        assert_eq!(fold("食べた"), "食べた");
        assert_eq!(fold("パスワード"), "パスワード");
    }
}
