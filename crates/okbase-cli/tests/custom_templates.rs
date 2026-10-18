//! Public template format, source discovery, Git provenance and untrusted input boundaries.
use std::path::{Path, PathBuf};
use std::process::Command as Git;

use assert_cmd::Command;
use serde_json::Value;

fn command(root: &Path, args: &[&str]) -> Command {
    let mut cmd = Command::cargo_bin("okbase").unwrap();
    cmd.args(["-b", root.to_str().unwrap()])
        .args(args)
        .arg("--json")
        .env("OKBASE_OFFLINE", "1")
        .write_stdin("")
        .timeout(std::time::Duration::from_secs(30));
    cmd
}
fn output(root: &Path, args: &[&str]) -> Value {
    let out = command(root, args).output().unwrap();
    assert!(out.status.success(), "{args:?}: {out:?}");
    serde_json::from_slice(&out.stdout).unwrap()
}
fn copy(from: &Path, to: &Path) {
    std::fs::create_dir_all(to).unwrap();
    for entry in std::fs::read_dir(from).unwrap() {
        let entry = entry.unwrap();
        let target = to.join(entry.file_name());
        if entry.file_type().unwrap().is_dir() {
            copy(&entry.path(), &target);
        } else {
            std::fs::copy(entry.path(), target).unwrap();
        }
    }
}
fn starter(to: &Path) {
    copy(
        &PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../templates/_starter"),
        to,
    );
}
fn reject(root: &Path, args: &[&str], message: &str) {
    let out = command(root, args).output().unwrap();
    assert!(!out.status.success(), "unexpected success: {out:?}");
    let text = format!(
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(text.contains(message), "{text}");
    assert!(!root.exists(), "invalid inputs must not create the target");
}
fn git(root: &Path, args: &[&str]) -> String {
    let out = Git::new("git")
        .arg("-C")
        .arg(root)
        .args([
            "-c",
            "user.name=Template test",
            "-c",
            "user.email=template@example.invalid",
            "-c",
            "commit.gpgsign=false",
            "-c",
            "core.autocrlf=false",
        ])
        .args(args)
        .output()
        .unwrap();
    assert!(out.status.success(), "{args:?}: {out:?}");
    String::from_utf8(out.stdout).unwrap().trim().to_owned()
}

#[test]
fn local_standard_preserves_documents_settings_and_user_extensions() {
    let tmp = tempfile::tempdir().unwrap();
    let source = tmp.path().join("custom template");
    starter(&source);
    let root = tmp.path().join("new bundle");
    let doc = source.join("bundle/guides/getting-started.md");
    let text = std::fs::read_to_string(&doc).unwrap().replace(
        "status: stable",
        "# Keep this comment\ncustom_field: [one, two]\nstatus: stable",
    );
    std::fs::write(&doc, &text).unwrap();
    let index = source.join("bundle/index.md");
    let root_text = std::fs::read_to_string(&index)
        .unwrap()
        .replace("okf_version:", "# Header comment\nokf_version:")
        .replace("\r\n", "\n")
        .replace('\n', "\r\n");
    std::fs::write(&index, &root_text).unwrap();
    let config = source.join("bundle/okbase.toml");
    std::fs::write(
        &config,
        "# Keep this too\n[bundle]\nprofile = \"okf\"\nlangs = [\"en\"]\n[custom]\nanswer = 42\n",
    )
    .unwrap();
    let preview = output(
        &root,
        &[
            "init",
            "--template-path",
            source.to_str().unwrap(),
            "--langs",
            "vi,en",
            "--types",
            "Decision,Guide",
            "--print",
        ],
    );
    assert!(!root.exists());
    assert_eq!(preview["template"], "my-template");
    assert!(preview["source"].as_str().unwrap().starts_with("local:"));
    let created = output(
        &root,
        &[
            "init",
            "--template-path",
            source.to_str().unwrap(),
            "--langs",
            "vi,en",
            "--types",
            "Decision,Guide",
        ],
    );
    assert_eq!(created["source"], preview["source"]);
    assert_eq!(
        std::fs::read_to_string(root.join("guides/getting-started.md")).unwrap(),
        text
    );
    assert_eq!(
        std::fs::read_to_string(root.join("index.md")).unwrap(),
        root_text
    );
    let actual = std::fs::read_to_string(root.join("okbase.toml")).unwrap();
    assert!(actual.contains("# Keep this too"));
    assert!(actual.contains("answer = 42"));
    assert!(actual.contains("langs = [\"vi\", \"en\"]"));
    assert_eq!(
        std::fs::read(root.join("_meta/types/Guide.md")).unwrap(),
        std::fs::read(source.join("bundle/_meta/types/Guide.md")).unwrap()
    );
    assert!(root.join("_meta/types/Decision.md").exists());
    assert!(!root.join("_meta/types/Policy.md").exists());
    assert_eq!(output(&root, &["lint", "--level", "L2"])["errors"], 0);
    assert!(!source.join(".okbase").exists());
}

#[test]
fn local_catalog_discovers_new_entries_and_selects_by_manifest_id() {
    let tmp = tempfile::tempdir().unwrap();
    let source = tmp.path().join("collection");
    starter(&source.join("one"));
    starter(&source.join("nested/two"));
    let m = source.join("nested/two/template.yaml");
    std::fs::write(
        &m,
        std::fs::read_to_string(&m)
            .unwrap()
            .replace("id: my-template", "id: another-template"),
    )
    .unwrap();
    let catalog = source.join("catalog.yaml");
    std::fs::write(
        &catalog,
        "format_version: 1\ntemplates:\n  - path: one\n  - path: nested/two\n",
    )
    .unwrap();
    let root = tmp.path().join("absent");
    let list = output(&root, &["templates", "--source", source.to_str().unwrap()]);
    assert_eq!(list["templates"][0]["id"], "my-template");
    assert_eq!(list["templates"][1]["id"], "another-template");
    reject(
        &root,
        &["init", "--template-path", source.to_str().unwrap()],
        "select a catalog entry",
    );
    let preview = output(
        &root,
        &[
            "init",
            "--template-path",
            source.to_str().unwrap(),
            "--template",
            "another-template",
            "--print",
        ],
    );
    assert_eq!(preview["template"], "another-template");
    std::fs::write(
        &catalog,
        "format_version: 1\ntemplates:\n  - path: one\n  - path: one\n",
    )
    .unwrap();
    reject(
        &root,
        &["templates", "--source", source.to_str().unwrap()],
        "duplicate template catalog path",
    );
    std::fs::write(
        &catalog,
        "format_version: 1\ntemplates:\n  - path: ../one\n",
    )
    .unwrap();
    reject(
        &root,
        &["templates", "--source", source.to_str().unwrap()],
        "unsafe template path",
    );
    std::fs::write(&catalog, "format_version: 9\ntemplates: []\n").unwrap();
    reject(
        &root,
        &["templates", "--source", source.to_str().unwrap()],
        "unsupported catalog",
    );
}

#[test]
fn malformed_manifests_and_invalid_bundles_fail_before_writing() {
    let tmp = tempfile::tempdir().unwrap();
    let source = tmp.path().join("template");
    starter(&source);
    let m = source.join("template.yaml");
    let original = std::fs::read_to_string(&m).unwrap();
    let root = tmp.path().join("absent");
    for (manifest, expected) in [
        (
            original.replace("format_version: 1", "format_version: 2"),
            "unsupported template",
        ),
        (
            format!("{original}\ninstall: arbitrary-command\n"),
            "unknown field",
        ),
        (
            original.replace("bundle: bundle", "bundle: ../outside"),
            "unsafe template path",
        ),
        (
            original.replace("id: my-template", "id: Bad Name"),
            "template id must",
        ),
        (
            format!(
                "{original}\nquestions:\n- question: Missing?\n  source: missing\n  evidence: absent\n"
            ),
            "no matching evidence",
        ),
    ] {
        std::fs::write(&m, manifest).unwrap();
        reject(
            &root,
            &[
                "init",
                "--template-path",
                source.to_str().unwrap(),
                "--print",
            ],
            expected,
        );
    }
    std::fs::write(&m, original).unwrap();
    let config = source.join("bundle/okbase.toml");
    let original_config = std::fs::read_to_string(&config).unwrap();
    std::fs::write(&config, "bundle = 42\n").unwrap();
    reject(
        &root,
        &[
            "init",
            "--template-path",
            source.to_str().unwrap(),
            "--langs",
            "vi",
        ],
        "must contain a [bundle] table",
    );
    std::fs::write(&config, original_config).unwrap();
    let doc = source.join("bundle/guides/getting-started.md");
    let text = std::fs::read_to_string(&doc)
        .unwrap()
        .replace("type: Guide", "type: MissingSchema");
    std::fs::write(doc, text).unwrap();
    reject(
        &root,
        &["init", "--template-path", source.to_str().unwrap()],
        "must meet L2",
    );
    let large = std::fs::File::create(source.join("bundle/oversized.txt")).unwrap();
    large.set_len(32 * 1024 * 1024 + 1).unwrap();
    reject(
        &root,
        &[
            "init",
            "--template-path",
            source.to_str().unwrap(),
            "--print",
        ],
        "32 MiB",
    );
}

#[test]
fn git_sources_resolve_commits_and_use_the_same_standard_without_checkout() {
    let tmp = tempfile::tempdir().unwrap();
    let source = tmp.path().join("repo");
    starter(&source.join("collection/custom"));
    std::fs::write(
        source.join("collection/catalog.yaml"),
        "format_version: 1\ntemplates:\n- path: custom\n",
    )
    .unwrap();
    git(&source, &["init", "--quiet"]);
    git(&source, &["add", "."]);
    git(&source, &["commit", "-qm", "Initial template"]);
    let first = git(&source, &["rev-parse", "HEAD"]);
    let hook = source.join(".git/hooks/post-checkout");
    std::fs::write(&hook, "#!/bin/sh\nprintf executed > \"$0.executed\"\n").unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&hook, std::fs::Permissions::from_mode(0o755)).unwrap();
    }

    let manifest = source.join("collection/custom/template.yaml");
    std::fs::write(
        &manifest,
        std::fs::read_to_string(&manifest)
            .unwrap()
            .replace("version: 1.0.0", "version: 2.0.0"),
    )
    .unwrap();
    git(&source, &["add", "."]);
    git(&source, &["commit", "-qm", "Update template"]);
    let root = tmp.path().join("target");
    let list = output(
        &root,
        &[
            "templates",
            "--repo",
            source.to_str().unwrap(),
            "--source",
            "collection",
            "--ref",
            &first,
        ],
    );
    assert_eq!(list["commit"], first);
    assert_eq!(list["templates"][0]["id"], "my-template");
    let preview = output(
        &root,
        &[
            "init",
            "--template-repo",
            source.to_str().unwrap(),
            "--template-path",
            "collection",
            "--template",
            "my-template",
            "--ref",
            &first,
            "--print",
        ],
    );
    assert!(!root.exists());
    let provenance = preview["source"].as_str().unwrap();
    assert!(
        provenance.contains(&format!("#{first}:collection/custom")),
        "{provenance}"
    );
    let log = preview["files"]
        .as_array()
        .unwrap()
        .iter()
        .find(|f| f["path"] == "log.md")
        .unwrap()["content"]
        .as_str()
        .unwrap();
    assert!(log.contains("version `1.0.0`"));
    assert!(log.contains(provenance));
    let created = output(
        &root,
        &[
            "init",
            "--template-repo",
            source.to_str().unwrap(),
            "--template-path",
            "collection/custom",
            "--ref",
            &first,
        ],
    );
    assert_eq!(created["source"], preview["source"]);
    assert!(!source.join(".git/hooks/post-checkout.executed").exists());
    assert!(!root.join(".git").exists());
    assert!(!root.join("template.yaml").exists());
    let absent = tmp.path().join("missing-ref");
    reject(
        &absent,
        &[
            "init",
            "--template-repo",
            source.to_str().unwrap(),
            "--ref",
            "does-not-exist",
        ],
        "Git template resolution failed",
    );
}

#[test]
fn git_root_template_and_offline_errors_are_explicit() {
    let tmp = tempfile::tempdir().unwrap();
    let repo = tmp.path().join("repo");
    starter(&repo);
    git(&repo, &["init", "--quiet"]);
    git(&repo, &["add", "."]);
    git(&repo, &["commit", "-qm", "Root template"]);
    let root = tmp.path().join("absent");
    assert_eq!(
        output(
            &root,
            &["init", "--template-repo", repo.to_str().unwrap(), "--print"]
        )["template"],
        "my-template"
    );
    reject(
        &root,
        &[
            "init",
            "--template-repo",
            "https://example.invalid/templates",
        ],
        "OKBASE_OFFLINE",
    );
    reject(
        &root,
        &["templates", "--repo", "git@example.invalid:templates.git"],
        "OKBASE_OFFLINE",
    );
    reject(
        &root,
        &[
            "templates",
            "--repo",
            "ssh://git@example.invalid:2222/templates.git",
        ],
        "OKBASE_OFFLINE",
    );

    reject(
        &root,
        &[
            "init",
            "--template-repo",
            "https://user:secret@example.invalid/templates",
        ],
        "without embedded credentials",
    );
    reject(
        &root,
        &[
            "init",
            "--template-repo",
            repo.to_str().unwrap(),
            "--ref=--upload-pack=evil",
        ],
        "--ref must",
    );
}

#[test]
#[cfg(unix)]
fn local_and_git_template_symlinks_are_rejected() {
    use std::os::unix::fs::symlink;
    let tmp = tempfile::tempdir().unwrap();
    let repo = tmp.path().join("repo");
    starter(&repo);
    let outside = tmp.path().join("outside.txt");
    std::fs::write(&outside, "private text").unwrap();
    symlink(&outside, repo.join("bundle/leak.md")).unwrap();
    let root = tmp.path().join("absent");
    reject(
        &root,
        &["init", "--template-path", repo.to_str().unwrap(), "--print"],
        "regular file or directory",
    );
    git(&repo, &["init", "--quiet"]);
    git(&repo, &["add", "."]);
    git(&repo, &["commit", "-qm", "Unsafe symlink"]);
    reject(
        &root,
        &["init", "--template-repo", repo.to_str().unwrap(), "--print"],
        "symlink or submodule",
    );
    assert_eq!(std::fs::read_to_string(outside).unwrap(), "private text");
}
