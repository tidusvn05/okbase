//! Lints a bundle: `cargo run -p okbase-lint --example lint -- <bundle> [L0|L1|L2|L3]`.
fn main() {
    let mut args = std::env::args().skip(1);
    let root = args.next().expect("usage: lint <bundle> [level]");
    let level = match args.next().as_deref() {
        Some("L0") => okbase_lint::Level::L0,
        Some("L1") => okbase_lint::Level::L1,
        Some("L3") => okbase_lint::Level::L3,
        _ => okbase_lint::Level::L2,
    };
    let report =
        okbase_lint::lint(root.as_ref(), &okbase_lint::LintConfig::level(level)).expect("lint");
    print!("{}", report.to_text());
}
