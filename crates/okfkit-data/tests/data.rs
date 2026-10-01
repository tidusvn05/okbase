//! Import and read-only SQL on the business fixture and a small XLSX.

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use okfkit_data::{Data, Error, Limits};

fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../fixtures")
        .join(name)
}

fn all(_: &str) -> bool {
    true
}

fn business() -> (tempfile::TempDir, Data) {
    let tmp = tempfile::tempdir().unwrap();
    let data = Data::new(&fixture("business"), &tmp.path().join("datasets.sqlite"));
    let r = data.sync().unwrap();
    assert_eq!(
        r.imported,
        [
            "data/inventory.csv",
            "data/price_list.csv",
            "data/sales_2026.csv"
        ]
    );
    (tmp, data)
}

#[test]
fn tables_and_aggregates() {
    let (_tmp, data) = business();
    let t = data.tables(&all).unwrap();
    insta::assert_snapshot!("business_tables", t.to_text());
    let sales = t.tables.iter().find(|t| t.name == "sales_2026").unwrap();
    assert_eq!(sales.rows, 506);
    assert_eq!(
        sales
            .columns
            .iter()
            .find(|c| c.name == "units")
            .unwrap()
            .sql_type,
        "INTEGER"
    );

    let r = data
        .query("SELECT region, currency, SUM(revenue) AS revenue FROM sales_2026 GROUP BY region, currency ORDER BY region;", &Limits::default(), &all)
        .unwrap();
    assert_eq!(r.columns, ["region", "currency", "revenue"]);
    assert_eq!(r.rows.len(), 2);
    assert!(
        r.to_text()
            .starts_with("| region | currency | revenue |\n| JP | JPY | "),
        "{}",
        r.to_text()
    );
    // _schema keeps the original headers.
    let s = data
        .query(
            "SELECT label FROM _schema WHERE table_name = 'inventory' ORDER BY position",
            &Limits::default(),
            &all,
        )
        .unwrap();
    assert_eq!(s.rows[0][0], "sku");
    // Row limit.
    let many = data
        .query(
            "SELECT * FROM inventory",
            &Limits {
                max_rows: 10,
                ..Default::default()
            },
            &all,
        )
        .unwrap();
    assert!(many.truncated && many.rows.len() == 10);
    // Second sync: nothing changes.
    let again = data.sync().unwrap();
    assert!(again.imported.is_empty() && again.removed.is_empty());
}

#[test]
fn only_reads_are_allowed() {
    let (_tmp, data) = business();
    for sql in [
        "DELETE FROM inventory",
        "INSERT INTO inventory (sku) VALUES ('x')",
        "PRAGMA table_info(inventory)",
        "ATTACH DATABASE '/tmp/x.db' AS x",
        "SELECT 1; DROP TABLE inventory",
        "SELECT * FROM _okfkit_sources",
        "SELECT * FROM sqlite_master",
        "",
    ] {
        let r = data.query(sql, &Limits::default(), &all);
        assert!(matches!(r, Err(Error::Sql(_))), "{sql:?} gave {r:?}");
    }
    let t = Instant::now();
    let r = data.query(
        "WITH RECURSIVE n(i) AS (SELECT 1 UNION ALL SELECT i + 1 FROM n) SELECT COUNT(*) FROM n",
        &Limits {
            timeout: Duration::from_millis(200),
            ..Default::default()
        },
        &all,
    );
    assert!(matches!(r, Err(Error::Timeout(_))), "{r:?}");
    assert!(t.elapsed() < Duration::from_secs(3));
}

#[test]
fn scope_hides_tables() {
    let (_tmp, data) = business();
    let no_prices = |p: &str| !p.starts_with("data/price_list");
    let t = data.tables(&no_prices).unwrap();
    assert!(t.tables.iter().all(|t| t.name != "price_list"));
    assert!(matches!(
        data.query(
            "SELECT COUNT(*) FROM price_list",
            &Limits::default(),
            &no_prices
        ),
        Err(Error::Sql(_))
    ));
    // The schema table would reveal hidden columns, so it is hidden too.
    assert!(
        data.query("SELECT * FROM _schema", &Limits::default(), &no_prices)
            .is_err()
    );
    assert!(
        data.query(
            "SELECT COUNT(*) FROM inventory",
            &Limits::default(),
            &no_prices
        )
        .is_ok()
    );
}

#[test]
fn xlsx_sheets_and_changes() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path().join("kb");
    std::fs::create_dir_all(root.join("data")).unwrap();
    std::fs::copy(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/data/stock.xlsx"),
        root.join("data/Stock.xlsx"),
    )
    .unwrap();
    std::fs::write(root.join("data/notes.tsv"), "Tên\tGiá trị\na\t007\n").unwrap();
    let data = Data::new(&root, &tmp.path().join("d.sqlite"));
    data.sync().unwrap();
    let t = data.tables(&all).unwrap();
    let names: Vec<_> = t.tables.iter().map(|t| t.name.as_str()).collect();
    assert_eq!(names, ["notes", "stock__kho_ha_noi", "stock__tokyo"]);
    let hn = &t.tables[1];
    assert_eq!(
        hn.columns
            .iter()
            .map(|c| (c.name.as_str(), c.sql_type.as_str()))
            .collect::<Vec<_>>(),
        [("ma_sp", "TEXT"), ("so_luong", "INTEGER"), ("gia", "REAL")]
    );
    assert_eq!(t.tables[0].columns[1].sql_type, "TEXT"); // leading zero kept as text
    let r = data
        .query(
            "SELECT SUM(so_luong * gia) FROM stock__kho_ha_noi",
            &Limits::default(),
            &all,
        )
        .unwrap();
    assert_eq!(r.rows[0][0], 21.0);

    std::fs::write(root.join("data/notes.tsv"), "Tên\tGiá trị\na\t1\nb\t2\n").unwrap();
    std::fs::remove_file(root.join("data/Stock.xlsx")).unwrap();
    let s = data.sync().unwrap();
    assert_eq!(
        (s.imported.as_slice(), s.removed.as_slice()),
        (
            ["data/notes.tsv".to_owned()].as_slice(),
            ["data/Stock.xlsx".to_owned()].as_slice()
        )
    );
    let names: Vec<_> = data
        .tables(&all)
        .unwrap()
        .tables
        .into_iter()
        .map(|t| t.name)
        .collect();
    assert_eq!(names, ["notes"]);
}

/// PLAN §11: an aggregate over 10k rows ≤ 50 ms. Run with `--release -- --ignored`.
#[test]
#[ignore = "timing test; run in release mode"]
fn perf_aggregate_10k_rows() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path().join("kb");
    std::fs::create_dir_all(&root).unwrap();
    let mut csv = String::from("month,region,sku,units,revenue\n");
    for i in 0..10_000 {
        csv.push_str(&format!(
            "2026-{:02},{},SKU-{},{},{}\n",
            i % 12 + 1,
            ["VN", "JP"][i % 2],
            i % 300,
            i % 97,
            i * 13
        ));
    }
    std::fs::write(root.join("sales.csv"), csv).unwrap();
    let data = Data::new(&root, &tmp.path().join("d.sqlite"));
    data.sync().unwrap();
    let t = Instant::now();
    let r = data
        .query("SELECT region, month, SUM(units), SUM(revenue) FROM sales GROUP BY region, month ORDER BY 4 DESC", &Limits::default(), &all)
        .unwrap();
    let took = t.elapsed();
    eprintln!(
        "aggregate over 10k rows: {} groups in {took:?}",
        r.rows.len()
    );
    assert!(took < Duration::from_millis(50), "{took:?}");
}
