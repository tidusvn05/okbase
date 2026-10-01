//! Fine-tuning workflow pieces that need no model: activation gate and exact rollback.

use okfkit::tune::{Settings, activate, record_exported_model, rollback};
use okfkit::{Bundle, OpenOptions, Scope, StateDir};

fn bundle(dir: &std::path::Path) -> Bundle {
    let b = Bundle::open(
        &dir.join("kb"),
        OpenOptions::default().state_dir(StateDir::Path(dir.join("state"))),
    )
    .unwrap();
    b.sync().unwrap();
    b
}

#[test]
fn activate_needs_a_passing_gate_and_rollback_restores_exactly() {
    let tmp = tempfile::tempdir().unwrap();
    let kb = tmp.path().join("kb");
    std::fs::create_dir_all(&kb).unwrap();
    std::fs::write(
        kb.join("a.md"),
        "---\ntitle: A\ndescription: Something about a.\n---\n\nSome text about the topic a, long enough to be a passage of its own for sampling purposes here.\n",
    )
    .unwrap();
    let b = bundle(tmp.path());
    let run = b
        .tune_init(
            Settings {
                min_heldout_docs: 0,
                heldout_share: 0.0,
                min_chunk_tokens: 1,
                ..Default::default()
            },
            &Scope::all(),
        )
        .unwrap();
    let toml = kb.join("okfkit.toml");

    // Not exported, not evaluated.
    assert!(
        activate(&b, &run, false)
            .unwrap_err()
            .to_string()
            .contains("export")
    );
    record_exported_model(&run, "custom:kb-1").unwrap();
    assert!(
        activate(&b, &run, false)
            .unwrap_err()
            .to_string()
            .contains("tune eval")
    );
    // A failed gate blocks; --force overrides.
    let gate = |passed: bool| {
        serde_json::json!({"gate": {
            "run": run.plan().id, "base": "embeddinggemma-300m-q4", "tuned": "custom:kb-1",
            "questions": "held-out", "r_at_1": [0.8, 0.9], "cross_r_at_1": null, "same_r_at_1": null,
            "general_r_at_1": [0.85, 0.86], "n": 10, "passed": passed, "reasons": []
        }})
        .to_string()
    };
    std::fs::write(run.dir().join("eval.json"), gate(false)).unwrap();
    assert!(
        activate(&b, &run, false)
            .unwrap_err()
            .to_string()
            .contains("failed")
    );
    assert!(!toml.exists());
    std::fs::write(run.dir().join("eval.json"), gate(true)).unwrap();
    assert_eq!(activate(&b, &run, false).unwrap(), "custom:kb-1");
    assert!(
        std::fs::read_to_string(&toml)
            .unwrap()
            .contains("custom:kb-1")
    );
    // The bundle had no okfkit.toml: rollback removes it again.
    rollback(&b).unwrap();
    assert!(!toml.exists());
    assert!(
        rollback(&b)
            .unwrap_err()
            .to_string()
            .contains("nothing to roll back")
    );

    // An existing file comes back byte for byte.
    let original = "# my settings\n[modules]\nembed = \"off\"  # keep\n";
    std::fs::write(&toml, original).unwrap();
    activate(&b, &run, false).unwrap();
    rollback(&b).unwrap();
    assert_eq!(std::fs::read_to_string(&toml).unwrap(), original);
}
