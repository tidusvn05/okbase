//! Read APIs on the fixtures: spike semantics, scope enforcement and output snapshots.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use okfkit_index::Index;
use okfkit_query::*;

fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../fixtures")
        .join(name)
}

fn indexed(name: &str) -> Index {
    let mut idx = Index::open_in_memory(&fixture(name)).unwrap();
    idx.sync().unwrap();
    idx
}

fn business() -> &'static std::sync::Mutex<Index> {
    static IDX: OnceLock<std::sync::Mutex<Index>> = OnceLock::new();
    IDX.get_or_init(|| std::sync::Mutex::new(indexed("business")))
}

fn ids(r: &QueryResult) -> Vec<&str> {
    r.docs.iter().map(|d| d.id.as_str()).collect()
}

fn q(filter: Filter) -> QueryRequest {
    QueryRequest {
        filter,
        limit: Some(500),
        ..Default::default()
    }
}

#[test]
fn query_matches_spike_semantics() {
    let idx = business().lock().unwrap_or_else(|e| e.into_inner());
    let all = Scope::all();
    let contracts = Filter {
        types: vec!["Contract".into()],
        active_on: Some("2026-01-15".into()),
        ..Default::default()
    };
    assert_eq!(query(&idx, &q(contracts), &all).unwrap().total, 18);
    // Spike counts + the 3 Dataset pages (sources/sheets), which are not in the spike's metadata.json.
    for (day, n) in [
        ("2025-06-01", 92 + 3),
        ("2026-01-01", 101 + 3),
        ("2026-09-01", 111 + 3),
    ] {
        let f = Filter {
            active_on: Some(day.into()),
            ..Default::default()
        };
        assert_eq!(
            query(&idx, &q(f), &all).unwrap().total,
            n,
            "active_on {day}"
        );
    }
    let mut fields = BTreeMap::new();
    fields.insert("region".to_owned(), vec!["VN".to_owned()]);
    let vn_stable = Filter {
        types: vec!["Policy".into()],
        status: vec!["stable".into()],
        fields: fields.clone(),
        ..Default::default()
    };
    assert_eq!(query(&idx, &q(vn_stable), &all).unwrap().total, 8);
    let warranty_vn = Filter {
        tags_all: vec!["warranty".into(), "VN".into()],
        ..Default::default()
    };
    assert_eq!(
        ids(&query(&idx, &q(warranty_vn), &all).unwrap()),
        [
            "policies/vn/warranty-air-purifier-v1",
            "policies/vn/warranty-air-purifier-v2",
            "policies/vn/warranty-rice-cooker-v1",
            "policies/vn/warranty-rice-cooker-v2"
        ]
    );
    let ja = Filter {
        lang: vec!["ja".into()],
        ..Default::default()
    };
    assert_eq!(query(&idx, &q(ja), &all).unwrap().total, 51);

    let sum = QueryRequest {
        filter: Filter {
            types: vec!["Contract".into()],
            fields,
            ..Default::default()
        },
        sum_field: Some("contract_value".into()),
        facets: vec!["customer".into(), "type".into()],
        count_only: true,
        ..Default::default()
    };
    let r = query(&idx, &sum, &all).unwrap();
    let s = r.sum.as_ref().unwrap();
    assert_eq!((s.value, s.count), (54_199_000_000.0, 17));
    assert_eq!(s.units.get("VND"), Some(&17));
    assert!(r.docs.is_empty() && r.facets["type"] == [("Contract".to_owned(), 17)]);

    // Accent-insensitive text search.
    let text = Filter {
        text: Some("bao hanh".into()),
        types: vec!["Policy".into()],
        ..Default::default()
    };
    assert!(query(&idx, &q(text), &all).unwrap().total >= 2);
    // Sorting: newest first.
    let sorted = QueryRequest {
        sort: Some("-updated".into()),
        limit: Some(3),
        ..Default::default()
    };
    let r = query(&idx, &sorted, &all).unwrap();
    let dates: Vec<_> = r.docs.iter().map(|d| d.updated.clone().unwrap()).collect();
    assert!(dates.windows(2).all(|w| w[0] >= w[1]) && r.more > 0);
}

#[test]
fn grep_is_accent_insensitive_and_scoped() {
    let idx = business().lock().unwrap_or_else(|e| e.into_inner());
    let req = GrepRequest {
        pattern: "doi tra|返品".into(),
        files_only: true,
        limit: 200,
        ..Default::default()
    };
    let r = grep(&idx, &req, &Scope::all()).unwrap();
    assert!(r.total_docs > 3, "{}", r.to_text(true));
    let hidden = Scope::all().deny("policies/**").unwrap();
    let r2 = grep(&idx, &req, &hidden).unwrap();
    assert!(r2.docs.iter().all(|d| !d.id.starts_with("policies/")));
    assert!(r2.total_docs < r.total_docs);

    let none = grep(
        &idx,
        &GrepRequest {
            pattern: "zzqqxx".into(),
            ..Default::default()
        },
        &Scope::all(),
    )
    .unwrap();
    assert!(none.to_text(false).starts_with("no match"));
    let literal = grep(
        &idx,
        &GrepRequest {
            pattern: "v2 (".into(),
            ..Default::default()
        },
        &Scope::all(),
    )
    .unwrap();
    assert!(literal.literal);
    // Frontmatter is searched too.
    let fm = GrepRequest {
        pattern: "^customer: \"?saigon".into(),
        files_only: true,
        ..Default::default()
    };
    assert!(grep(&idx, &fm, &Scope::all()).unwrap().total_docs >= 1);
    let filtered = GrepRequest {
        pattern: "warranty".into(),
        files_only: true,
        filter: Some(Filter {
            status: vec!["deprecated".into()],
            ..Default::default()
        }),
        ..Default::default()
    };
    let r = grep(&idx, &filtered, &Scope::all()).unwrap();
    assert!(
        r.total_docs > 0 && r.docs.iter().all(|d| !d.id.ends_with("-v2")),
        "{}",
        r.to_text(true)
    );
}

#[test]
fn scope_hides_documents_everywhere() {
    let idx = business().lock().unwrap_or_else(|e| e.into_inner());
    let scope = Scope::all()
        .deny("contracts/**")
        .unwrap()
        .filter(MetaFilter::NotIn("lang".into(), vec!["ja".into()]));
    let id = "contracts/acme-corp/supply-202504";
    assert!(matches!(
        get(
            &idx,
            &GetRequest {
                id: id.into(),
                ..Default::default()
            },
            &scope
        ),
        Err(Error::NotFound(_))
    ));
    assert!(
        get(
            &idx,
            &GetRequest {
                id: id.into(),
                ..Default::default()
            },
            &Scope::all()
        )
        .is_ok()
    );
    let r = query(
        &idx,
        &QueryRequest {
            limit: Some(1000),
            ..Default::default()
        },
        &scope,
    )
    .unwrap();
    assert!(
        r.docs
            .iter()
            .all(|d| !d.id.starts_with("contracts/") && d.lang.as_deref() != Some("ja"))
    );
    let root = list(&idx, ".", &scope).unwrap();
    assert_eq!(root.source, "generated");
    assert!(!root.content.contains("contracts"));
    assert!(matches!(
        list(&idx, "contracts", &scope),
        Err(Error::NotFound(_))
    ));
    let cat = catalog(&idx, &CatalogOptions::default(), &scope).unwrap();
    assert!(!cat.content.contains("[contracts/"));
    assert!(matches!(links(&idx, id, &scope), Err(Error::NotFound(_))));
    let s = stats(&idx, &scope).unwrap();
    assert_eq!(s.docs, r.total);
    assert!(!s.langs.contains_key("ja"));
}

#[test]
fn get_sections_and_budget() {
    let idx = indexed("openclaw-s");
    let all = Scope::all();
    let req = GetRequest {
        id: "/gateway/configuration-reference.md".into(),
        max_tokens: Some(300),
        ..Default::default()
    };
    let full = get(&idx, &req, &all).unwrap();
    assert!(
        full.truncated && !full.headings.is_empty(),
        "{} tokens",
        full.tokens
    );
    let sec = &full.headings[1];
    let part = get(
        &idx,
        &GetRequest {
            id: full.id.clone(),
            section: Some(sec.clone()),
            max_tokens: Some(50_000),
            ..Default::default()
        },
        &all,
    )
    .unwrap();
    assert_eq!(part.section.as_deref(), Some(sec.as_str()));
    assert!(!part.truncated && part.content.len() < full.tokens * 8);
    let missing = get(
        &idx,
        &GetRequest {
            id: full.id.clone(),
            section: Some("no such heading".into()),
            ..Default::default()
        },
        &all,
    )
    .unwrap();
    assert!(missing.to_text().starts_with("section not found"));
    let lines = get(
        &idx,
        &GetRequest {
            id: full.id.clone(),
            lines: Some("1-3".into()),
            ..Default::default()
        },
        &all,
    )
    .unwrap();
    assert_eq!(lines.content.split('\n').count(), 3);
}

#[test]
fn links_and_mode() {
    let idx = indexed("okf-official/acme_retail");
    let l = links(&idx, "metrics/revenue", &Scope::all()).unwrap();
    assert!(!l.outgoing.is_empty() || !l.backlinks.is_empty(), "{l:?}");
    assert_eq!(recommend_mode(&idx, &Scope::all()).unwrap(), Mode::Full);
    let big = business().lock().unwrap_or_else(|e| e.into_inner());
    assert_eq!(recommend_mode(&big, &Scope::all()).unwrap(), Mode::Lexical);
}

#[test]
fn snapshots() {
    let idx = indexed("okf-official/acme_retail");
    let all = Scope::all();
    let grep_req = GrepRequest {
        pattern: "net revenue|refund".into(),
        context: 0,
        limit: 25,
        ..Default::default()
    };
    insta::assert_snapshot!(
        "grep_acme",
        grep(&idx, &grep_req, &all).unwrap().to_text(false)
    );
    let files = GrepRequest {
        files_only: true,
        ..grep_req
    };
    insta::assert_snapshot!(
        "grep_acme_files",
        grep(&idx, &files, &all).unwrap().to_text(true)
    );
    insta::assert_snapshot!("list_acme_root", list(&idx, ".", &all).unwrap().content);
    insta::assert_snapshot!(
        "catalog_acme",
        catalog(&idx, &CatalogOptions::default(), &all)
            .unwrap()
            .content
    );
    let get_req = GetRequest {
        id: "metrics/revenue".into(),
        max_tokens: Some(200),
        ..Default::default()
    };
    insta::assert_snapshot!(
        "get_acme_revenue",
        get(&idx, &get_req, &all).unwrap().to_text()
    );
    insta::assert_json_snapshot!("stats_acme", stats(&idx, &all).unwrap());

    let biz = business().lock().unwrap_or_else(|e| e.into_inner());
    let req = QueryRequest {
        filter: Filter {
            types: vec!["Contract".into()],
            active_on: Some("2026-01-15".into()),
            ..Default::default()
        },
        facets: vec!["region".into()],
        sort: Some("-contract_value".into()),
        limit: Some(5),
        ..Default::default()
    };
    insta::assert_snapshot!(
        "query_contracts",
        query(&biz, &req, &all).unwrap().to_text()
    );
}

fn copy_dir(from: &Path, to: &Path) {
    std::fs::create_dir_all(to).unwrap();
    for e in std::fs::read_dir(from).unwrap() {
        let e = e.unwrap();
        if e.file_type().unwrap().is_dir() {
            copy_dir(&e.path(), &to.join(e.file_name()));
        } else if e.path().extension().is_some_and(|x| x == "md") {
            std::fs::copy(e.path(), to.join(e.file_name())).unwrap();
        }
    }
}

fn replicated(name: &str, copies: usize) -> (tempfile::TempDir, Index) {
    let tmp = tempfile::tempdir().unwrap();
    for i in 0..copies {
        copy_dir(&fixture(name), &tmp.path().join(format!("c{i:03}")));
    }
    let mut idx = Index::open_in_memory(tmp.path()).unwrap();
    idx.sync().unwrap();
    (tmp, idx)
}

/// PLAN §11: grep over 4.4M tokens ≤ 500 ms. Run with `--release -- --ignored`.
#[test]
#[ignore = "timing test; run in release mode"]
fn perf_grep_4_4m_tokens() {
    let (_tmp, idx) = replicated("openclaw-s", 51);
    let tokens = stats(&idx, &Scope::all()).unwrap().tokens;
    assert!(tokens >= 4_400_000, "only {tokens} tokens");
    for pattern in ["heartbeat|dmScope", "config.*gateway", "đổi trả"] {
        let req = GrepRequest {
            pattern: pattern.into(),
            files_only: true,
            ..Default::default()
        };
        let start = std::time::Instant::now();
        let r = grep(&idx, &req, &Scope::all()).unwrap();
        let took = start.elapsed();
        eprintln!(
            "grep {pattern:?} over {tokens} tokens: {} docs in {took:?}",
            r.total_docs
        );
        assert!(took.as_millis() <= 500, "{took:?}");
    }
}

/// PLAN §11: query over 3k documents ≤ 20 ms. Run with `--release -- --ignored`.
#[test]
#[ignore = "timing test; run in release mode"]
fn perf_query_3k_docs() {
    let (_tmp, idx) = replicated("business", 20);
    let req = QueryRequest {
        filter: Filter {
            types: vec!["Contract".into()],
            active_on: Some("2026-01-15".into()),
            ..Default::default()
        },
        facets: vec!["region".into(), "customer".into()],
        sum_field: Some("contract_value".into()),
        ..Default::default()
    };
    let n = query(
        &idx,
        &QueryRequest {
            count_only: true,
            ..Default::default()
        },
        &Scope::all(),
    )
    .unwrap()
    .total;
    assert!(n >= 3000, "{n}");
    let start = std::time::Instant::now();
    let r = query(&idx, &req, &Scope::all()).unwrap();
    let took = start.elapsed();
    eprintln!("query over {n} docs: {} matches in {took:?}", r.total);
    let s2 = std::time::Instant::now();
    let _ = query(
        &idx,
        &QueryRequest {
            count_only: true,
            ..Default::default()
        },
        &Scope::all(),
    )
    .unwrap();
    eprintln!("second (count only): {:?}", s2.elapsed());
    assert_eq!(r.total, 18 * 20);
    assert!(took.as_millis() <= 20, "{took:?}");
}
