//! The lexical eval as a test: no case that passed when recorded may fail now.

use std::path::Path;

#[test]
fn no_regressions_on_the_fixture_suites() {
    let fixtures = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures");
    let mut suites = 0;
    for entry in std::fs::read_dir(fixtures.join("eval")).expect("fixtures/eval") {
        let path = entry.expect("entry").path();
        if path.extension().is_none_or(|e| e != "json") {
            continue;
        }
        let report = okfkit_eval::Suite::load(&path)
            .and_then(|s| s.run(&fixtures))
            .expect("suite runs");
        let failing: Vec<_> = report
            .cases
            .iter()
            .filter(|c| report.regressions.contains(&c.id))
            .map(|c| format!("{}: {}", c.id, c.missing.join("; ")))
            .collect();
        assert!(
            failing.is_empty(),
            "{} regressions (run `cargo run -p okfkit-eval -- lexical`):\n{}",
            report.suite,
            failing.join("\n")
        );
        suites += 1;
    }
    assert!(suites >= 2);
}
