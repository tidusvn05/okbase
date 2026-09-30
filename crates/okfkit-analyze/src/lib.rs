//! Text analysis for okfkit: Unicode folding, Vietnamese accent stripping,
//! Japanese tokenization, English stemming and language hints.
//!
//! - [`fold`] makes text comparable across accents, width and case. `grep` uses it.
//! - [`Analyzer`] turns text into search terms for the full-text index.
//! - [`detect_lang`] guesses vi/en/ja. It is a hint only (unaccented Vietnamese is
//!   recognized about 90% of the time in the spikes).
//!
//! Cargo features: `ja` (lindera with the embedded IPADIC dictionary) and
//! `detect` (lingua), both on by default. Without `ja`, Japanese and Chinese text
//! is indexed as overlapping character bigrams.

mod analyzer;
mod detect;
mod fold;
pub mod tokens;

pub use analyzer::{Analyzer, is_cjk};
pub use detect::{Lang, detect_lang};
pub use fold::fold;
pub use tokens::estimate_tokens;
