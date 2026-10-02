//! Concept model, byte-identical frontmatter round-trip, OKF v0.2 validation and
//! link extraction for [okfkit](https://github.com/okfkit/okfkit).
//!
//! ```
//! use okfkit_core::Concept;
//! use serde_json::json;
//! use std::path::Path;
//!
//! let text = "---\ntype: Metric  # kept\ntags: [sales]\n---\n# Revenue\n";
//! let mut c = Concept::parse(Path::new("metrics/revenue.md"), text).unwrap();
//! assert_eq!(c.render(), text); // unchanged documents round-trip byte for byte
//!
//! c.frontmatter.set("tags", json!(["sales", "finance"])).unwrap();
//! assert_eq!(c.render(), "---\ntype: Metric  # kept\ntags: [sales, finance]\n---\n# Revenue\n");
//! ```

mod concept;
mod error;
pub mod frontmatter;
pub mod id;
pub mod links;
pub mod validate;

pub use concept::{Concept, DEFAULT_EXCLUDES, IGNORE_FILE, discover, walk};
pub use error::Error;
pub use frontmatter::{Frontmatter, FrontmatterState, Mapping};
pub use id::ConceptId;
pub use links::{Link, LinkKind, extract_links, resolve_link, resolve_wikilink};
pub use validate::{Issue, Severity, validate};

/// A parsed YAML value. Mappings keep their source key order.
pub type Value = serde_json::Value;
