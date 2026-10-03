//! Per-type field schemas in `_meta/types/<Type>.md`.
//!
//! A schema is an OKF document whose frontmatter declares the fields of one
//! concept type. The body is free prose.
//!
//! ```markdown
//! ---
//! type: Type Schema
//! title: Policy
//! applies_to: Policy          # optional; defaults to the file name
//! fields:
//!   owner: {type: string, required: true}
//!   region: {type: string, enum: [VN, JP]}
//!   effective_from: {type: date}
//!   contract_value: number    # shorthand: just the type
//! ---
//! ```
//!
//! Field types: `string`, `number`, `integer`, `bool`, `date` (YYYY-MM-DD, time
//! allowed), `list`, `map`, `any`.

use std::collections::BTreeMap;
use std::path::Path;

use okbase_core::{Concept, Value, discover};

use crate::Error;

/// Directory of type schemas, relative to the bundle root.
pub const TYPES_DIR: &str = "_meta/types";

/// The kind of value a field holds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FieldType {
    /// Any string.
    String,
    /// Any number.
    Number,
    /// A whole number.
    Integer,
    /// `true` or `false`.
    Bool,
    /// An ISO date (`YYYY-MM-DD`, optionally followed by a time).
    Date,
    /// A YAML list.
    List,
    /// A YAML mapping.
    Map,
    /// Anything.
    Any,
}

impl FieldType {
    fn parse(s: &str) -> Option<Self> {
        Some(match s.trim().to_ascii_lowercase().as_str() {
            "string" | "text" => FieldType::String,
            "number" | "float" => FieldType::Number,
            "integer" | "int" => FieldType::Integer,
            "bool" | "boolean" => FieldType::Bool,
            "date" | "datetime" => FieldType::Date,
            "list" | "array" => FieldType::List,
            "map" | "object" => FieldType::Map,
            "any" => FieldType::Any,
            _ => return None,
        })
    }

    /// Whether `v` has this type.
    pub fn accepts(self, v: &Value) -> bool {
        match self {
            FieldType::Any => true,
            FieldType::String => v.is_string(),
            FieldType::Number => v.is_number(),
            FieldType::Integer => v.is_i64() || v.is_u64(),
            FieldType::Bool => v.is_boolean(),
            FieldType::Date => v.as_str().is_some_and(crate::level::is_iso_date),
            FieldType::List => v.is_array(),
            FieldType::Map => v.is_object(),
        }
    }

    /// The name used in schemas.
    pub fn name(self) -> &'static str {
        match self {
            FieldType::String => "string",
            FieldType::Number => "number",
            FieldType::Integer => "integer",
            FieldType::Bool => "bool",
            FieldType::Date => "date",
            FieldType::List => "list",
            FieldType::Map => "map",
            FieldType::Any => "any",
        }
    }
}

/// One declared field.
#[derive(Debug, Clone, PartialEq)]
pub struct FieldSpec {
    /// Expected type.
    pub field_type: FieldType,
    /// Whether concepts of the type must have the field.
    pub required: bool,
    /// Allowed values (compared as strings, case-sensitive), if restricted.
    pub allowed: Option<Vec<String>>,
}

/// The schema of one concept type.
#[derive(Debug, Clone, PartialEq)]
pub struct TypeSchema {
    /// The concept `type` it applies to.
    pub applies_to: String,
    /// Bundle-relative path of the schema document.
    pub path: String,
    /// Declared fields, by name.
    pub fields: BTreeMap<String, FieldSpec>,
}

/// A problem with one field of a concept.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FieldProblem {
    /// A required field is absent.
    Missing,
    /// The value has the wrong type (expected type name).
    WrongType(&'static str),
    /// The value is not one of the allowed values.
    NotAllowed(Vec<String>),
}

impl TypeSchema {
    /// Checks a concept's frontmatter against the schema. Returns `(field, problem)` pairs.
    pub fn check(&self, fm: &okbase_core::Frontmatter) -> Vec<(String, FieldProblem)> {
        let mut out = Vec::new();
        for (name, spec) in &self.fields {
            let Some(v) = fm.get(name).filter(|v| !v.is_null()) else {
                if spec.required {
                    out.push((name.clone(), FieldProblem::Missing));
                }
                continue;
            };
            if !spec.field_type.accepts(v) {
                out.push((
                    name.clone(),
                    FieldProblem::WrongType(spec.field_type.name()),
                ));
            } else if let Some(allowed) = &spec.allowed {
                let values: Vec<&Value> = match v {
                    Value::Array(items) => items.iter().collect(),
                    v => vec![v],
                };
                let text = |v: &Value| v.as_str().map_or_else(|| v.to_string(), str::to_owned);
                if values.iter().any(|x| !allowed.contains(&text(x))) {
                    out.push((name.clone(), FieldProblem::NotAllowed(allowed.clone())));
                }
            }
        }
        out
    }
}

/// Loads every schema under `_meta/types/`, keyed by the type they apply to.
pub fn load_type_schemas(root: &Path) -> Result<BTreeMap<String, TypeSchema>, Error> {
    let dir = root.join(TYPES_DIR);
    let mut out = BTreeMap::new();
    if !dir.is_dir() {
        return Ok(out);
    }
    for rel in discover(&dir)? {
        let rel = Path::new(TYPES_DIR).join(rel);
        let doc = Concept::read(root, &rel)?;
        if doc.is_reserved() {
            continue;
        }
        let schema = parse_schema(&doc)?;
        out.insert(schema.applies_to.clone(), schema);
    }
    Ok(out)
}

/// Parses a schema document.
pub fn parse_schema(doc: &Concept) -> Result<TypeSchema, Error> {
    let invalid = |m: String| Error::InvalidMeta {
        path: doc.path.clone(),
        message: m,
    };
    let fm = &doc.frontmatter;
    if let Some(e) = fm.error() {
        return Err(invalid(e.to_owned()));
    }
    let applies_to = fm
        .get_str("applies_to")
        .map(str::to_owned)
        .unwrap_or_else(|| doc.id.name().to_owned());
    let mut fields = BTreeMap::new();
    match fm.get("fields") {
        None | Some(Value::Null) => {}
        Some(Value::Object(map)) => {
            for (name, def) in map {
                let spec = match def {
                    Value::String(t) => FieldSpec {
                        field_type: FieldType::parse(t).ok_or_else(|| {
                            invalid(format!("field `{name}`: unknown type `{t}`"))
                        })?,
                        required: false,
                        allowed: None,
                    },
                    Value::Object(d) => {
                        let t = d.get("type").and_then(Value::as_str).unwrap_or("any");
                        let allowed = match d.get("enum") {
                            None => None,
                            Some(Value::Array(items)) => Some(
                                items
                                    .iter()
                                    .map(|v| {
                                        v.as_str().map_or_else(|| v.to_string(), str::to_owned)
                                    })
                                    .collect(),
                            ),
                            Some(_) => {
                                return Err(invalid(format!(
                                    "field `{name}`: `enum` must be a list"
                                )));
                            }
                        };
                        FieldSpec {
                            field_type: FieldType::parse(t).ok_or_else(|| {
                                invalid(format!("field `{name}`: unknown type `{t}`"))
                            })?,
                            required: d.get("required").and_then(Value::as_bool).unwrap_or(false),
                            allowed,
                        }
                    }
                    _ => {
                        return Err(invalid(format!(
                            "field `{name}`: expected a type name or a mapping"
                        )));
                    }
                };
                fields.insert(name.clone(), spec);
            }
        }
        Some(_) => return Err(invalid("`fields` must be a mapping".into())),
    }
    Ok(TypeSchema {
        applies_to,
        path: doc.path.clone(),
        fields,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use okbase_core::Frontmatter;

    #[test]
    fn schema_checks() {
        let doc = Concept::parse(
            Path::new("_meta/types/Policy.md"),
            "---\ntype: Type Schema\nfields:\n  owner: {type: string, required: true}\n  region: {type: string, enum: [VN, JP]}\n  effective_from: date\n  n: integer\n---\n",
        )
        .unwrap();
        let s = parse_schema(&doc).unwrap();
        assert_eq!(s.applies_to, "Policy");
        let fm = Frontmatter::split(
            "---\ntype: Policy\nregion: US\neffective_from: soon\nn: 1.5\n---\n",
        )
        .0;
        let got = s.check(&fm);
        assert_eq!(
            got,
            [
                ("effective_from".into(), FieldProblem::WrongType("date")),
                ("n".into(), FieldProblem::WrongType("integer")),
                ("owner".into(), FieldProblem::Missing),
                (
                    "region".into(),
                    FieldProblem::NotAllowed(vec!["VN".into(), "JP".into()])
                ),
            ]
        );
        let ok = Frontmatter::split(
            "---\ntype: Policy\nowner: human:a\nregion: VN\neffective_from: 2026-01-01\n---\n",
        )
        .0;
        assert!(s.check(&ok).is_empty());
    }
}
