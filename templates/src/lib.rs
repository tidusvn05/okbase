//! Built-in template assets generated from the root catalog at compile time.
//! Add a template directory and catalog entry; no Rust registry edits are needed.
#![forbid(unsafe_code)]

/// Versioned catalog, in declared display order.
pub const CATALOG: &str = include_str!("../catalog.yaml");

include!(concat!(env!("OUT_DIR"), "/assets.rs"));
