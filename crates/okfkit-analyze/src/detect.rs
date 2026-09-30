/// A language okfkit recognizes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Lang {
    /// Vietnamese.
    Vi,
    /// English.
    En,
    /// Japanese.
    Ja,
}

impl Lang {
    /// The ISO 639-1 code (`vi`, `en`, `ja`).
    pub fn code(self) -> &'static str {
        match self {
            Lang::Vi => "vi",
            Lang::En => "en",
            Lang::Ja => "ja",
        }
    }
}

/// Letters that only occur in Vietnamese among vi/en/ja.
fn has_vietnamese_letters(s: &str) -> bool {
    s.chars()
        .any(|c| "ăâđêôơưĂÂĐÊÔƠƯạảấầẩẫậắằẳẵặẹẻẽếềểễệỉịọỏốồổỗộớờởỡợụủứừửữựỳỵỷỹ".contains(c))
}

/// Guesses whether `text` is Vietnamese, English or Japanese. A hint only.
///
/// Kana or ideographs mean Japanese and Vietnamese-only letters mean Vietnamese.
/// Otherwise lingua decides (feature `detect`); without it, text defaults to English.
/// Returns `None` for text without letters.
pub fn detect_lang(text: &str) -> Option<Lang> {
    if !text.chars().any(char::is_alphabetic) {
        return None;
    }
    if text.chars().any(crate::is_cjk) {
        return Some(Lang::Ja);
    }
    if has_vietnamese_letters(text) {
        return Some(Lang::Vi);
    }
    lingua_detect(text)
}

#[cfg(feature = "detect")]
fn lingua_detect(text: &str) -> Option<Lang> {
    use std::sync::LazyLock;

    use lingua::{Language, LanguageDetector, LanguageDetectorBuilder};

    static DETECTOR: LazyLock<LanguageDetector> = LazyLock::new(|| {
        LanguageDetectorBuilder::from_languages(&[
            Language::English,
            Language::Vietnamese,
            Language::Japanese,
        ])
        .build()
    });
    match DETECTOR.detect_language_of(text)? {
        Language::Vietnamese => Some(Lang::Vi),
        Language::Japanese => Some(Lang::Ja),
        _ => Some(Lang::En),
    }
}

#[cfg(not(feature = "detect"))]
fn lingua_detect(_text: &str) -> Option<Lang> {
    Some(Lang::En)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects() {
        assert_eq!(detect_lang("Chính sách đổi trả hàng"), Some(Lang::Vi));
        assert_eq!(detect_lang("返品ポリシー"), Some(Lang::Ja));
        assert_eq!(
            detect_lang("How do refunds work for damaged items?"),
            Some(Lang::En)
        );
        assert_eq!(detect_lang("123 !!"), None);
        assert_eq!(Lang::Vi.code(), "vi");
    }

    #[cfg(feature = "detect")]
    #[test]
    fn unaccented_vietnamese() {
        assert_eq!(
            detect_lang("chinh sach doi tra hang trong vong bao nhieu ngay"),
            Some(Lang::Vi)
        );
    }
}
