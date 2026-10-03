//! SARIF 2.1.0 rendering.

use serde_json::{Value, json};

use crate::{CATALOG, Report, Severity};

pub(crate) fn to_sarif(report: &Report) -> Value {
    let rules: Vec<Value> = CATALOG
        .iter()
        .filter(|r| r.level <= report.target)
        .map(|r| {
            json!({
                "id": r.id,
                "shortDescription": {"text": r.description},
                "defaultConfiguration": {"level": level(r.severity)},
                "properties": {"okbaseLevel": r.level.to_string()},
            })
        })
        .collect();
    let results: Vec<Value> = report
        .diagnostics
        .iter()
        .map(|d| {
            json!({
                "ruleId": d.rule,
                "ruleIndex": CATALOG.iter().filter(|r| r.level <= report.target).position(|r| r.id == d.rule),
                "level": level(d.severity),
                "message": {"text": d.message},
                "locations": [{
                    "physicalLocation": {
                        "artifactLocation": {"uri": d.path, "uriBaseId": "BUNDLE"},
                        "region": {"startLine": d.line.unwrap_or(1)},
                    }
                }],
            })
        })
        .collect();
    json!({
        "$schema": "https://json.schemastore.org/sarif-2.1.0.json",
        "version": "2.1.0",
        "runs": [{
            "tool": {"driver": {
                "name": "okbase",
                "version": env!("CARGO_PKG_VERSION"),
                "informationUri": "https://github.com/tidusvn05/okbase",
                "rules": rules,
            }},
            "results": results,
        }],
    })
}

fn level(s: Severity) -> &'static str {
    match s {
        Severity::Error => "error",
        Severity::Warning => "warning",
    }
}
