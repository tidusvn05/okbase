//! Offline acceptance of the shipped starter bundles through the real CLI and read APIs.

use std::path::Path;

use assert_cmd::Command;
use okbase::{Bundle, GetRequest, GrepRequest, Scope};
use serde_json::{Value, json};

fn command(root: &Path, args: &[&str]) -> Command {
    let mut cmd = Command::cargo_bin("okbase").unwrap();
    cmd.arg("-b")
        .arg(root)
        .args(args)
        .env("OKBASE_OFFLINE", "1")
        .write_stdin("")
        .timeout(std::time::Duration::from_secs(30));
    cmd
}

fn output(root: &Path, args: &[&str]) -> Value {
    let out = command(root, args).arg("--json").output().unwrap();
    assert!(out.status.success(), "{args:?}: {out:?}");
    serde_json::from_slice(&out.stdout).unwrap()
}

#[test]
fn catalog_is_discoverable_offline_without_a_bundle_or_state() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path().join("does not exist");
    let catalog = output(&root, &["templates"]);
    assert_eq!(catalog["templates"].as_array().unwrap().len(), 10);
    insta::assert_snapshot!(
        "template_catalog",
        serde_json::to_string_pretty(&catalog).unwrap()
    );
    assert!(!root.exists());
    let help = command(&root, &["init", "--help"]).output().unwrap();
    let help = String::from_utf8(help.stdout).unwrap();
    for flag in ["--template", "--print", "okbase templates"] {
        assert!(help.contains(flag), "{help}");
    }
}

#[test]
fn every_template_reaches_l2_and_answers_its_five_trial_questions() {
    let tmp = tempfile::tempdir().unwrap();
    let scope = Scope::all();
    for entry in okbase_adopt::templates::catalog().unwrap() {
        let root = tmp.path().join(&entry.id);
        let created = output(&root, &["init", "--template", &entry.id]);
        assert_eq!(created["template"], entry.id);
        assert!(
            created["next"]
                .as_array()
                .unwrap()
                .iter()
                .all(|c| c.as_str().unwrap().contains(&root.display().to_string()))
        );
        let lint = output(&root, &["lint", "--level", "L3"]);
        assert_eq!(lint["errors"], 0, "{}: {lint:#}", entry.id);
        assert_eq!(lint["warnings"], 0, "{}: {lint:#}", entry.id);
        assert_eq!(lint["level"], "L2", "{}: {lint:#}", entry.id);

        let bundle = Bundle::open_in_memory(&root).unwrap();
        bundle.sync().unwrap();
        let template = okbase_adopt::templates::load(&entry.id).unwrap();
        assert_eq!(template.questions.len(), 5);
        for q in &template.questions {
            let source = bundle
                .get(
                    &GetRequest {
                        id: q.source.clone(),
                        ..Default::default()
                    },
                    &scope,
                )
                .unwrap();
            assert!(
                source.content.contains(&q.evidence),
                "{}: {}: {}",
                entry.id,
                q.question,
                source.content
            );
            assert_eq!(source.frontmatter["lang"], "en");
            assert_eq!(source.frontmatter["example"], true);
            assert_eq!(source.frontmatter["status"], "draft");
            let hits = bundle
                .grep(
                    &GrepRequest {
                        pattern: regex_escape(&q.evidence),
                        path: Some(format!("{}.md", q.source)),
                        files_only: true,
                        ..Default::default()
                    },
                    &scope,
                )
                .unwrap();
            assert_eq!(hits.total_docs, 1, "{}: {}: {hits:?}", entry.id, q.question);
            assert_eq!(hits.docs[0].id, q.source);
        }
        let catalog = output(&root, &["catalog"]);
        assert!(catalog.to_string().contains("Getting started"));
        let workflows = output(
            &root,
            &["query", "--type", "Guide", "--under", "workflows/"],
        );
        assert_eq!(workflows["total"], 3, "{}: {workflows}", entry.id);
        let graph = output(&root, &["graph"]);
        assert!(
            graph["nodes"]
                .as_array()
                .unwrap()
                .iter()
                .all(|node| node["broken"] == 0),
            "{}: {graph}",
            entry.id
        );
    }
}

fn regex_escape(text: &str) -> String {
    text.chars()
        .flat_map(|c| {
            if ".+*?()|[]{}^$\\".contains(c) {
                vec!['\\', c]
            } else {
                vec![c]
            }
        })
        .collect()
}

#[test]
fn preview_overrides_and_extra_types_preserve_the_english_examples() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path().join("my knowledge");
    let args = [
        "init",
        "--template",
        "project-assistant",
        "--title",
        "Atlas: \"\u{1f31f}\"",
        "--description",
        "User-owned project knowledge.",
        "--langs",
        "vi,en",
        "--types",
        "Custom,Task,Custom",
    ];
    let mut preview_args = args.to_vec();
    preview_args.push("--print");
    let preview = output(&root, &preview_args);
    assert!(
        !root.exists(),
        "preview creates neither the bundle nor its parents"
    );
    assert!(preview.get("created").is_none());
    let files = preview["files"].as_array().unwrap();
    assert!(files.iter().any(|f| f["path"] == "_meta/types/Custom.md"));
    let paths: Vec<_> = files.iter().map(|f| f["path"].clone()).collect();
    insta::assert_snapshot!(
        "project_preview_paths",
        serde_json::to_string_pretty(&paths).unwrap()
    );
    let created = output(&root, &args);
    assert_eq!(created["created"].as_array().unwrap().len(), files.len());
    for file in files {
        let text = std::fs::read_to_string(root.join(file["path"].as_str().unwrap())).unwrap();
        assert_eq!(text, file["content"].as_str().unwrap());
    }
    let config = std::fs::read_to_string(root.join("okbase.toml")).unwrap();
    assert!(config.contains(r#"langs = ["vi", "en"]"#));
    let index = std::fs::read_to_string(root.join("index.md")).unwrap();
    assert!(index.contains("Atlas: \"\u{1f31f}\""));
    let task = output(&root, &["get", "tasks/at-43"]);
    assert_eq!(task["frontmatter"]["lang"], "en");
    assert_eq!(task["frontmatter"]["task_status"], "blocked");
}

#[test]
fn task_states_inventory_and_host_scopes_use_existing_read_contracts() {
    let tmp = tempfile::tempdir().unwrap();
    let project = tmp.path().join("project");
    output(&project, &["init", "--template", "project-assistant"]);
    let blocked = output(
        &project,
        &["query", "--type", "Task", "--field", "task_status=blocked"],
    );
    assert_eq!(blocked["total"], 1);
    assert_eq!(blocked["docs"][0]["id"], "tasks/at-43");
    let bundle = Bundle::open_in_memory(&project).unwrap();
    bundle.sync().unwrap();
    let server = okbase_mcp::KbServer::new(
        bundle,
        std::sync::Arc::new(Scope::all()),
        &Default::default(),
    );
    let (_, mcp) = server
        .call(
            "kb_query",
            json!({"type": ["Task"], "fields": {"task_status": ["blocked"]}})
                .as_object()
                .unwrap()
                .clone(),
        )
        .unwrap();
    assert_eq!(
        blocked, mcp,
        "template queries use the same CLI and MCP contract"
    );
    let created = output(
        &project,
        &[
            "new",
            "--type",
            "Task",
            "Review rollout",
            "--description",
            "Review production evidence before closing the milestone.",
            "--tags",
            "task",
        ],
    );
    assert_eq!(
        created["to_fill"],
        json!(["owner", "project_id", "task_status"])
    );
    assert!(
        std::fs::read_to_string(project.join("tasks/index.md"))
            .unwrap()
            .contains("review-rollout.md")
    );
    let path = Path::new("tasks/review-rollout.md");
    let mut task = okbase::core::Concept::read(&project, path).unwrap();
    for (field, value) in [
        ("owner", "reviewer"),
        ("project_id", "atlas"),
        ("task_status", "todo"),
    ] {
        task.frontmatter.set(field, json!(value)).unwrap();
    }
    task.body
        .push_str("Review the source evidence and record the acceptance outcome.\n");
    std::fs::write(project.join(path), task.render()).unwrap();
    assert_eq!(output(&project, &["lint", "--level", "L2"])["errors"], 0);

    let operations = tmp.path().join("operations");
    output(&operations, &["init", "--template", "infra-ops"]);
    let services = output(
        &operations,
        &[
            "query",
            "--type",
            "Service",
            "--field",
            "server_id=app-01",
            "--field",
            "environment=production",
        ],
    );
    assert_eq!(services["total"], 2);
    let server = output(&operations, &["get", "servers/app-01"]);
    assert!(
        server["content"]
            .as_str()
            .unwrap()
            .contains("atlas-api and atlas-worker")
    );

    let personal = tmp.path().join("personal");
    output(&personal, &["init", "--template", "personal-assistant"]);
    let bundle = Bundle::open_in_memory(&personal).unwrap();
    bundle.sync().unwrap();
    let private = Scope::all().deny("profile/**").unwrap();
    assert!(
        bundle
            .get(
                &GetRequest {
                    id: "profile/preferences".into(),
                    ..Default::default()
                },
                &private
            )
            .is_err()
    );
    assert!(
        bundle
            .get(
                &GetRequest {
                    id: "profile/preferences".into(),
                    ..Default::default()
                },
                &Scope::all()
            )
            .is_ok()
    );
}

#[test]
fn init_rejects_conflicts_before_creating_any_files_and_keeps_the_old_skeleton() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path().join("bundle");
    for args in [
        vec!["init", "--template", "unknown"],
        vec![
            "init",
            "--template",
            "team-handbook",
            "--types",
            "../escape",
        ],
    ] {
        let out = command(&root, &args).arg("--json").output().unwrap();
        assert_eq!(out.status.code(), Some(1));
        let value: Value = serde_json::from_slice(&out.stdout).unwrap();
        assert_eq!(value["error"]["code"], "invalid_argument");
        assert!(!root.exists());
    }
    std::fs::create_dir(&root).unwrap();
    std::fs::write(root.join("okbase.toml"), "# user config\n").unwrap();
    command(&root, &["init", "--template", "team-handbook"])
        .assert()
        .failure();
    assert_eq!(std::fs::read_dir(&root).unwrap().count(), 1);
    assert_eq!(
        std::fs::read_to_string(root.join("okbase.toml")).unwrap(),
        "# user config\n"
    );
    std::fs::remove_file(root.join("okbase.toml")).unwrap();
    let preview = output(&root, &["init", "--title", "Old skeleton", "--print"]);
    assert_eq!(preview["files"].as_array().unwrap().len(), 7);
    let created = output(&root, &["init", "--title", "Old skeleton"]);
    assert!(created.get("template").is_none());
    assert!(root.join("_meta/types/Policy.md").is_file());
    let before = std::fs::read(root.join("index.md")).unwrap();
    command(&root, &["init", "--template", "software-project"])
        .assert()
        .failure();
    assert_eq!(std::fs::read(root.join("index.md")).unwrap(), before);
    assert!(!root.join("guides").exists());
    command(&tmp.path().join("missing"), &["init"])
        .assert()
        .code(2);
}

#[cfg(unix)]
#[test]
fn template_destinations_cannot_follow_symlinks_outside_the_bundle() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path().join("bundle");
    let outside = tmp.path().join("outside");
    std::fs::create_dir(&root).unwrap();
    std::fs::create_dir(&outside).unwrap();
    std::os::unix::fs::symlink(&outside, root.join("_meta")).unwrap();
    for extra in [vec![], vec!["--print"]] {
        let mut args = vec!["init", "--template", "infra-ops"];
        args.extend(extra);
        command(&root, &args).assert().failure();
        assert_eq!(std::fs::read_dir(&root).unwrap().count(), 1);
        assert_eq!(std::fs::read_dir(&outside).unwrap().count(), 0);
    }
}

#[test]
fn empty_folder_onboarding_offers_every_template_as_an_explicit_choice() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path().join("empty");
    std::fs::create_dir(&root).unwrap();
    // Relative bundle paths keep the command snapshot identical on Unix and Windows,
    // including when the system temporary directory needs shell quoting.
    let out = command(Path::new("empty"), &["onboard"])
        .current_dir(tmp.path())
        .arg("--json")
        .output()
        .unwrap();
    assert!(out.status.success(), "{out:?}");
    let plan: Value = serde_json::from_slice(&out.stdout).unwrap();
    let init = &plan["steps"][0];
    assert_eq!(init["id"], "init");
    assert_eq!(init["kind"], "ask");
    assert_eq!(init["options"].as_array().unwrap().len(), 12);
    for template in okbase_adopt::templates::catalog().unwrap() {
        assert!(
            init["options"]
                .to_string()
                .contains(&format!("--template {}", template.id))
        );
    }
    assert_eq!(std::fs::read_dir(&root).unwrap().count(), 0);
    let normalized = serde_json::to_string_pretty(init).unwrap();
    insta::assert_snapshot!("template_onboarding", normalized);
}
