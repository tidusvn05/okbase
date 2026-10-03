//! Property tests: random frontmatter round-trips, and `set`/`remove` touch only one key.

use okbase_core::{Frontmatter, Value};
use proptest::prelude::*;
use serde_json::json;

fn key() -> impl Strategy<Value = String> {
    prop_oneof![
        "[a-z][a-z0-9_]{0,8}",
        "[a-z]{1,4} [a-z]{1,4}".prop_map(|k| format!("\"{k}\"")),
        "[a-z]{1,4}-[a-z]{1,4}",
    ]
}

fn scalar() -> impl Strategy<Value = String> {
    prop_oneof![
        "[A-Za-z][A-Za-z0-9 ]{0,12}[A-Za-z0-9]",
        any::<i32>().prop_map(|n| n.to_string()),
        Just("true".to_owned()),
        Just("~".to_owned()),
        "[a-z ]{0,10}".prop_map(|s| format!("\"{s}: x\"")),
        "[a-z]{1,6}".prop_map(|s| format!("'{s}''s'")),
        Just("2026-05-28T14:30:00Z".to_owned()),
    ]
}

/// A top-level entry rendered in one of several YAML styles.
fn entry() -> impl Strategy<Value = (String, String)> {
    let comment = prop_oneof![
        Just(String::new()),
        "[a-z ]{0,10}".prop_map(|c| format!(" # {c}"))
    ];
    (
        key(),
        0..6u8,
        prop::collection::vec(scalar(), 0..4),
        comment,
    )
        .prop_map(|(k, style, items, c)| {
            let first = items.first().cloned().unwrap_or_else(|| "x".into());
            let v = match style {
                0 => format!(" {first}{c}\n"),
                1 => format!(" [{}]{c}\n", items.join(", ")),
                2 if !items.is_empty() => format!(
                    "{c}\n{}",
                    items
                        .iter()
                        .map(|i| format!("  - {i}\n"))
                        .collect::<String>()
                ),
                3 if !items.is_empty() => format!(
                    "\n{}",
                    items.iter().map(|i| format!("- {i}\n")).collect::<String>()
                ),
                4 => format!(" |\n  {first}\n\n  more{c}\n"),
                _ => format!("\n  by: {first}\n  # nested comment\n  at: x\n"),
            };
            (k, v)
        })
}

/// A document and its keys as `(spelling in the source, parsed name)`.
fn document() -> impl Strategy<Value = (String, Vec<(String, String)>)> {
    let filler = prop_oneof![
        Just(""),
        Just("\n"),
        Just("# a comment\n"),
        Just("\n# spaced\n\n")
    ];
    prop::collection::vec((entry(), filler), 1..8).prop_map(|entries| {
        let mut seen = std::collections::HashSet::new();
        let mut text = String::from("---\n");
        let mut keys = Vec::new();
        for ((k, v), f) in entries {
            if seen.insert(k.clone()) {
                text.push_str(f);
                text.push_str(&k);
                text.push(':');
                text.push_str(&v);
                keys.push((k.clone(), k.trim_matches('"').to_owned()));
            }
        }
        text.push_str("---\n# Body\n");
        (text, keys)
    })
}

fn new_value() -> impl Strategy<Value = Value> {
    prop_oneof![
        "[ -~]{0,20}".prop_map(Value::from),
        any::<i64>().prop_map(Value::from),
        prop::collection::vec("[a-z :]{0,6}", 0..4).prop_map(|v| json!(v)),
        ("[a-z]{0,6}", any::<bool>()).prop_map(|(s, b)| json!({"s": s, "b": b})),
        Just(Value::Null),
    ]
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(400))]

    #[test]
    fn round_trip((text, _keys) in document()) {
        let (fm, body) = Frontmatter::split(&text);
        prop_assert_eq!(fm.render() + body, text);
    }

    #[test]
    fn set_touches_one_key((text, keys) in document(), pick in any::<prop::sample::Index>(), fresh in any::<bool>(), value in new_value()) {
        let (orig, body) = Frontmatter::split(&text);
        prop_assume!(orig.mapping().is_some_and(|m| m.len() == keys.len()));
        let i = pick.index(keys.len());
        let key = if fresh { "zz_new".to_owned() } else { keys[i].1.clone() };
        let mut fm = orig.clone();
        fm.set(&key, value.clone()).unwrap();
        prop_assert_eq!(fm.get(&key), Some(&value));
        let others: Vec<_> = orig.keys().filter(|k| *k != key).collect();
        prop_assert!(fm.keys().filter(|k| *k != key).eq(others.iter().copied()));
        for k in &others {
            prop_assert_eq!(fm.get(k), orig.get(k));
        }
        let (old, new) = (orig.raw(), fm.raw());
        if fresh {
            prop_assert!(new.starts_with(old), "append changed existing text");
        } else {
            // Text before the key's line and from the next key's line on is untouched.
            let line_of = |spelling: &str| {
                let pat = format!("{spelling}:");
                if old.starts_with(&pat) { 0 } else { old.find(&format!("\n{pat}")).unwrap() + 1 }
            };
            let start = line_of(&keys[i].0);
            prop_assert!(new.starts_with(&old[..start]));
            if let Some(next) = keys.get(i + 1) {
                let from = line_of(&next.0);
                prop_assert!(new.ends_with(&old[from..]));
            }
        }
        let mut removed = fm.clone();
        prop_assert!(removed.remove(&key).unwrap());
        prop_assert!(removed.get(&key).is_none());
        prop_assert!(removed.render().len() + body.len() > 0);
    }
}
