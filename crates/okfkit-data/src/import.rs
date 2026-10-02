//! Discovering dataset files and importing them into SQLite.

use std::collections::{BTreeMap, HashSet};
use std::path::Path;

use calamine::{Data as Cell, Reader, open_workbook_auto};
use rusqlite::{Connection, params};

use crate::{Error, SCHEMA_TABLE, SOURCES_TABLE, SyncReport};

const EXTENSIONS: [&str; 3] = ["csv", "tsv", "xlsx"];

/// Bundle-relative paths (`/`-separated) of dataset files, sorted. Same skipping rules as the
/// markdown walk: hidden entries, dependency folders, `.gitignore` and `.okfkitignore`.
pub(crate) fn discover(root: &Path) -> Vec<String> {
    okfkit_core::walk(root, &EXTENSIONS)
        .unwrap_or_default()
        .into_iter()
        .map(|p| p.to_string_lossy().replace('\\', "/"))
        .collect()
}

/// One sheet of raw cells.
struct Sheet {
    name: Option<String>,
    headers: Vec<String>,
    rows: Vec<Vec<String>>,
}

fn read_sheets(path: &Path, rel: &str) -> Result<Vec<Sheet>, Error> {
    let err = |message: String| Error::Read {
        path: rel.to_owned(),
        message,
    };
    let ext = rel
        .rsplit_once('.')
        .map(|(_, e)| e.to_ascii_lowercase())
        .unwrap_or_default();
    if ext == "xlsx" {
        let mut wb = open_workbook_auto(path).map_err(|e| err(e.to_string()))?;
        let mut sheets = Vec::new();
        for name in wb.sheet_names().to_vec() {
            let range = wb.worksheet_range(&name).map_err(|e| err(e.to_string()))?;
            let mut rows = range
                .rows()
                .map(|r| r.iter().map(cell_text).collect::<Vec<_>>());
            let Some(headers) = rows.next() else { continue };
            sheets.push(Sheet {
                name: Some(name),
                headers,
                rows: rows.collect(),
            });
        }
        return Ok(sheets);
    }
    let mut reader = csv::ReaderBuilder::new()
        .delimiter(if ext == "tsv" { b'\t' } else { b',' })
        .flexible(true)
        .from_path(path)
        .map_err(|e| err(e.to_string()))?;
    let headers: Vec<String> = reader
        .headers()
        .map_err(|e| err(e.to_string()))?
        .iter()
        .map(|h| h.trim_start_matches('\u{feff}').to_owned())
        .collect();
    let mut rows = Vec::new();
    for rec in reader.records() {
        let rec = rec.map_err(|e| err(e.to_string()))?;
        rows.push(rec.iter().map(str::to_owned).collect());
    }
    Ok(vec![Sheet {
        name: None,
        headers,
        rows,
    }])
}

fn cell_text(c: &Cell) -> String {
    match c {
        Cell::Empty => String::new(),
        Cell::String(s) => s.clone(),
        Cell::Float(f) if f.fract() == 0.0 && f.abs() < 1e15 => format!("{}", *f as i64),
        Cell::Float(f) => f.to_string(),
        Cell::Int(i) => i.to_string(),
        Cell::Bool(b) => b.to_string(),
        Cell::DateTime(d) if !d.is_duration() => excel_date(d.as_f64()),
        Cell::DateTime(d) => d.as_f64().to_string(),
        Cell::DateTimeIso(s) | Cell::DurationIso(s) => s.clone(),
        Cell::Error(e) => format!("#{e:?}"),
    }
}

/// Excel serial date (days since 1899-12-30) → `YYYY-MM-DD` or `YYYY-MM-DD HH:MM:SS`.
fn excel_date(serial: f64) -> String {
    let days = serial.floor() as i64 - 25_569; // 1970-01-01
    // Civil date from days since the Unix epoch (H. Hinnant's algorithm).
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + i64::from(m <= 2);
    let secs = ((serial - serial.floor()) * 86_400.0).round() as i64;
    if secs == 0 {
        format!("{y:04}-{m:02}-{d:02}")
    } else {
        format!(
            "{y:04}-{m:02}-{d:02} {:02}:{:02}:{:02}",
            secs / 3600,
            secs / 60 % 60,
            secs % 60
        )
    }
}

/// `Tên khách hàng` → `ten_khach_hang`; empty → `col`.
pub(crate) fn sql_name(s: &str) -> String {
    let folded = okfkit_analyze::fold(s);
    let mut out = String::new();
    for c in folded.chars() {
        if c.is_ascii_alphanumeric() {
            out.push(c);
        } else if !out.ends_with('_') {
            out.push('_');
        }
    }
    let out = out.trim_matches('_').to_owned();
    match out.chars().next() {
        None => "col".into(),
        Some(c) if c.is_ascii_digit() => format!("c_{out}"),
        _ => out,
    }
}

fn unique(name: String, taken: &mut HashSet<String>) -> String {
    let mut candidate = name.clone();
    let mut n = 2;
    while !taken.insert(candidate.clone()) {
        candidate = format!("{name}_{n}");
        n += 1;
    }
    candidate
}

/// `INTEGER` if every non-empty value is an integer (no leading zeros), `REAL` if a number, else `TEXT`.
fn infer(values: impl Iterator<Item = String>) -> &'static str {
    let mut ty = "INTEGER";
    let mut any = false;
    for v in values {
        let v = v.trim();
        if v.is_empty() {
            continue;
        }
        any = true;
        let leading_zero = v.len() > 1 && v.starts_with('0') && !v.starts_with("0.");
        if ty == "INTEGER" && (leading_zero || v.parse::<i64>().is_err()) {
            ty = "REAL";
        }
        if ty == "REAL" && (leading_zero || v.parse::<f64>().map_or(true, |f| !f.is_finite())) {
            return "TEXT";
        }
    }
    if any { ty } else { "TEXT" }
}

fn quote(id: &str) -> String {
    format!("\"{}\"", id.replace('"', "\"\""))
}

pub(crate) fn sync(root: &Path, conn: &mut Connection) -> Result<SyncReport, Error> {
    conn.execute_batch(&format!(
        "CREATE TABLE IF NOT EXISTS {SOURCES_TABLE} (table_name TEXT PRIMARY KEY, source TEXT NOT NULL, sheet TEXT, rows INTEGER NOT NULL, hash TEXT NOT NULL);
         CREATE TABLE IF NOT EXISTS {SCHEMA_TABLE} (table_name TEXT NOT NULL, position INTEGER NOT NULL, column_name TEXT NOT NULL, label TEXT NOT NULL, type TEXT NOT NULL, source TEXT NOT NULL, sheet TEXT);"
    ))?;
    let known: BTreeMap<String, String> = {
        let mut st = conn.prepare(&format!(
            "SELECT DISTINCT source, hash FROM {SOURCES_TABLE}"
        ))?;
        st.query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?
            .collect::<Result<_, _>>()?
    };
    let files = discover(root);
    let mut report = SyncReport::default();
    let tx = conn.transaction()?;
    let drop_source = |tx: &rusqlite::Transaction, source: &str| -> Result<(), Error> {
        let names: Vec<String> = {
            let mut st = tx.prepare(&format!(
                "SELECT table_name FROM {SOURCES_TABLE} WHERE source = ?1"
            ))?;
            st.query_map([source], |r| r.get(0))?
                .collect::<Result<_, _>>()?
        };
        for n in names {
            tx.execute_batch(&format!("DROP TABLE IF EXISTS {};", quote(&n)))?;
        }
        tx.execute(
            &format!("DELETE FROM {SOURCES_TABLE} WHERE source = ?1"),
            [source],
        )?;
        tx.execute(
            &format!("DELETE FROM {SCHEMA_TABLE} WHERE source = ?1"),
            [source],
        )?;
        Ok(())
    };
    for source in known.keys().filter(|s| !files.contains(s)) {
        drop_source(&tx, source)?;
        report.removed.push(source.clone());
    }
    let mut taken: HashSet<String> = {
        let mut st = tx.prepare(&format!("SELECT table_name FROM {SOURCES_TABLE}"))?;
        st.query_map([], |r| r.get(0))?.collect::<Result<_, _>>()?
    };
    taken.insert(SCHEMA_TABLE.to_owned());
    taken.insert(SOURCES_TABLE.to_owned());
    for rel in &files {
        let path = root.join(rel);
        let bytes = match std::fs::read(&path) {
            Ok(b) => b,
            Err(e) => {
                report.skipped.push((rel.clone(), e.to_string()));
                continue;
            }
        };
        let hash = blake3::hash(&bytes).to_hex().to_string();
        if known.get(rel) == Some(&hash) {
            continue;
        }
        if known.contains_key(rel) {
            let names: Vec<String> = {
                let mut st = tx.prepare(&format!(
                    "SELECT table_name FROM {SOURCES_TABLE} WHERE source = ?1"
                ))?;
                st.query_map([rel], |r| r.get(0))?
                    .collect::<Result<_, _>>()?
            };
            for n in &names {
                taken.remove(n);
            }
            drop_source(&tx, rel)?;
        }
        let sheets = match read_sheets(&path, rel) {
            Ok(s) => s,
            Err(e) => {
                report.skipped.push((rel.clone(), e.to_string()));
                continue;
            }
        };
        let stem = sql_name(
            rel.rsplit('/')
                .next()
                .unwrap_or(rel)
                .rsplit_once('.')
                .map_or(rel.as_str(), |(s, _)| s),
        );
        let multi = sheets.len() > 1;
        for sheet in sheets {
            let base = match (&sheet.name, multi) {
                (Some(n), true) => format!("{stem}__{}", sql_name(n)),
                _ => stem.clone(),
            };
            let table = unique(base, &mut taken);
            let width = sheet
                .headers
                .len()
                .max(sheet.rows.iter().map(Vec::len).max().unwrap_or(0));
            let mut col_taken = HashSet::new();
            let cols: Vec<(String, String, &str)> = (0..width)
                .map(|i| {
                    let label = sheet.headers.get(i).cloned().unwrap_or_default();
                    let name = unique(
                        if label.trim().is_empty() {
                            format!("col_{}", i + 1)
                        } else {
                            sql_name(&label)
                        },
                        &mut col_taken,
                    );
                    let ty = infer(
                        sheet
                            .rows
                            .iter()
                            .map(|r| r.get(i).cloned().unwrap_or_default()),
                    );
                    (name, label, ty)
                })
                .collect();
            let defs: Vec<String> = cols
                .iter()
                .map(|(n, _, t)| format!("{} {t}", quote(n)))
                .collect();
            tx.execute_batch(&format!(
                "CREATE TABLE {} ({});",
                quote(&table),
                defs.join(", ")
            ))?;
            {
                let marks: Vec<String> = (1..=cols.len()).map(|i| format!("?{i}")).collect();
                let mut ins = tx.prepare(&format!(
                    "INSERT INTO {} VALUES ({})",
                    quote(&table),
                    marks.join(", ")
                ))?;
                for row in &sheet.rows {
                    if row.iter().all(|c| c.trim().is_empty()) {
                        continue;
                    }
                    let values: Vec<rusqlite::types::Value> = cols
                        .iter()
                        .enumerate()
                        .map(|(i, (_, _, ty))| {
                            let v = row.get(i).map(|s| s.trim()).unwrap_or("");
                            match (*ty, v) {
                                (_, "") => rusqlite::types::Value::Null,
                                ("INTEGER", v) => v.parse::<i64>().map_or(
                                    rusqlite::types::Value::Null,
                                    rusqlite::types::Value::Integer,
                                ),
                                ("REAL", v) => v.parse::<f64>().map_or(
                                    rusqlite::types::Value::Null,
                                    rusqlite::types::Value::Real,
                                ),
                                (_, v) => rusqlite::types::Value::Text(v.to_owned()),
                            }
                        })
                        .collect();
                    ins.execute(rusqlite::params_from_iter(values))?;
                }
            }
            let count: i64 = tx.query_row(
                &format!("SELECT COUNT(*) FROM {}", quote(&table)),
                [],
                |r| r.get(0),
            )?;
            tx.execute(
                &format!("INSERT INTO {SOURCES_TABLE} (table_name, source, sheet, rows, hash) VALUES (?1, ?2, ?3, ?4, ?5)"),
                params![table, rel, sheet.name, count, hash],
            )?;
            for (i, (n, label, ty)) in cols.iter().enumerate() {
                tx.execute(
                    &format!("INSERT INTO {SCHEMA_TABLE} (table_name, position, column_name, label, type, source, sheet) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)"),
                    params![table, i as i64, n, label, ty, rel, sheet.name],
                )?;
            }
        }
        report.imported.push(rel.clone());
    }
    tx.commit()?;
    Ok(report)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_and_types() {
        assert_eq!(sql_name("Tên khách hàng"), "ten_khach_hang");
        assert_eq!(sql_name("2026 Q1"), "c_2026_q1");
        assert_eq!(sql_name("  "), "col");
        let s = |v: &[&str]| infer(v.iter().map(|x| (*x).to_owned()));
        assert_eq!(s(&["1", "", "-3"]), "INTEGER");
        assert_eq!(s(&["1", "2.5"]), "REAL");
        assert_eq!(s(&["007", "1"]), "TEXT");
        assert_eq!(s(&["a"]), "TEXT");
        assert_eq!(s(&[""]), "TEXT");
        assert_eq!(excel_date(46_023.0), "2026-01-01");
        assert_eq!(excel_date(46_023.5), "2026-01-01 12:00:00");
    }
}
