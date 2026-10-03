//! The okbase standard on top of OKF v0.2: quality levels L0–L2, the mapping of
//! common non-OKF frontmatter (Mintlify, Jekyll, Hugo, Docusaurus) to OKF fields,
//! and the tag vocabulary in `_meta/vocabulary.md`.
//!
//! Everything here is read-only: mapped fields are computed when reading and are
//! never written back to files.

mod error;
pub mod level;
pub mod mapping;
pub mod types;
pub mod vocabulary;

pub use error::Error;
pub use level::{Assessment, Finding, Level, assess, assess_bundle, assess_profile, is_iso_date};
pub use mapping::{Mapped, Meta, meta};
pub use types::{FieldProblem, FieldSpec, FieldType, TYPES_DIR, TypeSchema, load_type_schemas};
pub use vocabulary::{Term, Vocabulary, normalize_tag};

/// Default bundle-relative path of the tag vocabulary.
pub const VOCABULARY_PATH: &str = "_meta/vocabulary.md";

/// Directory for okbase metadata documents (vocabulary, type schemas).
pub const META_DIR: &str = "_meta";
