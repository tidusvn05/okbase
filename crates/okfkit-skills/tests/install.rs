//! Install plans for Claude Code and Codex, applied in a temporary home and project.

use std::fs;
use std::path::Path;

use okfkit_skills::*;

fn opts(agent: Agent, target: Target, home: &Path, bundle: &Path) -> InstallOptions {
    InstallOptions {
        agent,
        target,
        home: home.to_owned(),
        command: "/usr/local/bin/okfkit".into(),
        server_name: "okfkit".into(),
        skill: SkillContext {
            bundle: bundle.to_owned(),
            prefix: "kb".into(),
            capabilities: vec!["read.grep".into(), "read.query".into()],
        },
    }
}

#[test]
fn rendered_skill_snapshot() {
    let cx = SkillContext {
        bundle: "/srv/kb".into(),
        prefix: "kb".into(),
        capabilities: vec!["read.grep".into()],
    };
    let skills = builtin_skills(&cx);
    assert_eq!(
        skills.iter().map(|s| s.name.as_str()).collect::<Vec<_>>(),
        [
            "okfkit-answer",
            "okfkit-curate",
            "okfkit-adopt",
            "okfkit-tune"
        ]
    );
    insta::assert_snapshot!("okfkit_answer_lexical", skills[0].content);
    insta::assert_snapshot!("okfkit_curate", skills[1].content);
    insta::assert_snapshot!("okfkit_adopt", skills[2].content);
    insta::assert_snapshot!("okfkit_tune", skills[3].content);
    for s in &skills {
        assert!(
            !s.content.contains("<!--") && !s.content.contains("{{"),
            "{}",
            s.name
        );
    }
}

#[test]
fn claude_project_install() {
    let tmp = tempfile::tempdir().unwrap();
    let (home, project, bundle) = (
        tmp.path().join("home"),
        tmp.path().join("proj"),
        tmp.path().join("kb"),
    );
    fs::create_dir_all(bundle.join("_meta/skills/team-style")).unwrap();
    fs::write(
        bundle.join("_meta/skills/team-style/SKILL.md"),
        "---\nname: team-style\ndescription: House style for {{bundle}}.\n---\nBe brief.\n",
    )
    .unwrap();
    fs::create_dir_all(&project).unwrap();
    fs::write(
        project.join(".mcp.json"),
        r#"{"mcpServers": {"other": {"command": "x"}}}"#,
    )
    .unwrap();

    let actions = plan(&opts(
        Agent::Claude,
        Target::Project(project.clone()),
        &home,
        &bundle,
    ))
    .unwrap();
    assert!(actions.iter().all(|a| matches!(a, Action::Write { .. })));
    assert!(!project.join(".claude").exists(), "plan must not write");
    apply(&actions).unwrap();

    let mcp: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(project.join(".mcp.json")).unwrap()).unwrap();
    assert_eq!(mcp["mcpServers"]["other"]["command"], "x");
    assert_eq!(
        mcp["mcpServers"]["okfkit"]["command"],
        "/usr/local/bin/okfkit"
    );
    assert_eq!(
        mcp["mcpServers"]["okfkit"]["args"][1],
        bundle.display().to_string()
    );
    let skill = fs::read_to_string(project.join(".claude/skills/okfkit-answer/SKILL.md")).unwrap();
    assert!(skill.starts_with("---\nname: okfkit-answer\n"));
    let team = fs::read_to_string(project.join(".claude/skills/team-style/SKILL.md")).unwrap();
    assert!(team.contains(&format!("House style for {}.", bundle.display())));
    // Re-planning after install changes nothing.
    let again = plan(&opts(
        Agent::Claude,
        Target::Project(project.clone()),
        &home,
        &bundle,
    ))
    .unwrap();
    for a in again {
        if let Action::Write { path, content, .. } = a {
            assert_eq!(
                fs::read_to_string(&path).unwrap(),
                content,
                "{}",
                path.display()
            );
        }
    }
}

#[test]
fn claude_user_install_uses_claude_mcp_add() {
    let tmp = tempfile::tempdir().unwrap();
    let actions = plan(&opts(
        Agent::Claude,
        Target::User,
        tmp.path(),
        Path::new("/kb"),
    ))
    .unwrap();
    let Action::Run { argv, .. } = &actions[0] else {
        panic!("expected a command")
    };
    assert_eq!(
        argv[..6],
        ["claude", "mcp", "add", "--scope", "user", "okfkit"]
    );
    assert!(
        matches!(&actions[1], Action::Write { path, .. } if path.ends_with(".claude/skills/okfkit-answer/SKILL.md"))
    );
    assert!(actions[0].to_string().contains("$ claude mcp add --scope user okfkit -- /usr/local/bin/okfkit --bundle /kb mcp serve --stdio"));
}

#[test]
fn codex_install_preserves_config_and_agents_md() {
    let tmp = tempfile::tempdir().unwrap();
    let (home, project) = (tmp.path().join("home"), tmp.path().join("proj"));
    fs::create_dir_all(home.join(".codex")).unwrap();
    fs::create_dir_all(&project).unwrap();
    let config =
        "# my settings\nmodel = \"gpt-5\"\n\n[mcp_servers.other]\ncommand = \"x\" # keep me\n";
    fs::write(home.join(".codex/config.toml"), config).unwrap();
    fs::write(project.join("AGENTS.md"), "# Rules\n\nBe nice.\n").unwrap();

    let o = opts(
        Agent::Codex,
        Target::Project(project.clone()),
        &home,
        Path::new("/kb"),
    );
    apply(&plan(&o).unwrap()).unwrap();
    let written = fs::read_to_string(home.join(".codex/config.toml")).unwrap();
    assert!(written.starts_with(config), "{written}");
    assert!(written.contains("[mcp_servers.okfkit]\ncommand = \"/usr/local/bin/okfkit\"\nargs = [\"--bundle\", \"/kb\", \"mcp\", \"serve\", \"--stdio\"]\n"), "{written}");
    let agents = fs::read_to_string(project.join("AGENTS.md")).unwrap();
    assert!(agents.starts_with("# Rules\n\nBe nice.\n\n<!-- okfkit:begin"));
    assert!(
        agents.contains("# Answering from the knowledge bundle")
            && !agents.contains("name: okfkit-answer")
    );
    // Installing twice gives the same files.
    apply(&plan(&o).unwrap()).unwrap();
    assert_eq!(
        fs::read_to_string(project.join("AGENTS.md")).unwrap(),
        agents
    );
    assert_eq!(
        fs::read_to_string(home.join(".codex/config.toml")).unwrap(),
        written
    );
}

#[test]
fn broken_config_is_not_touched() {
    let tmp = tempfile::tempdir().unwrap();
    let project = tmp.path().join("p");
    fs::create_dir_all(&project).unwrap();
    fs::write(project.join(".mcp.json"), "{not json").unwrap();
    let err = plan(&opts(
        Agent::Claude,
        Target::Project(project),
        tmp.path(),
        Path::new("/kb"),
    ))
    .unwrap_err();
    assert!(matches!(err, Error::Config { .. }));
}
