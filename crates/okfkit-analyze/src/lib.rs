//! Text analysis for okfkit: Unicode folding, Vietnamese accent stripping,
//! Japanese tokenization, English stemming and language hints.
//!
//! - [`fold`] makes text comparable across accents, width and case. `grep` uses it.
//! - [`Analyzer`] turns text into search terms for the full-text index.
//! - [`detect_lang`] guesses vi/en/ja. It is a hint only (unaccented Vietnamese is
//!   recognized about 90% of the time in the spikes).
//!
//! Cargo features: `ja` (lindera; the IPADIC dictionary is loaded from the user
//! cache, see [`dict`]), `ja-download` (download it on first use) and `detect`
//! (lingua), all on by default; `ja-embedded` embeds the dictionary instead.
//! Without a dictionary, Japanese and Chinese text is indexed as overlapping
//! character bigrams.

mod analyzer;
mod detect;
pub mod dict;
mod fold;
pub mod tokens;

pub use analyzer::{Analyzer, cjk_mode, is_cjk};
pub use detect::{Lang, detect_lang};
pub use fold::fold;
pub use tokens::estimate_tokens;
