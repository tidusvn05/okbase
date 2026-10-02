//! Install plans for Claude Code and Codex, applied in a temporary home and project.

use std::fs;
use std::path::Path;

use okfkit_skills::*;

fn opts(agent: Agent, target: Target, home: &Path, bundle: &Path) -> InstallOptions {
    InstallOptions {
        agent,
        target,
        home: home.to_owned(),
        server: Server::Stdio {
            command: "/usr/local/bin/okfkit".into(),
        },
        server_name: "okfkit".into(),
        skill: SkillContext {
            bundle: bundle.to_owned(),
            prefix: "kb".into(),
            capabilities: vec!["read.grep".into(), "read.query".into()],
            allow: vec![],
            deny: vec![],
        },
        replace: false,
    }
}

#[test]
fn rendered_skill_snapshot() {
    let cx = SkillContext {
        bundle: "/srv/kb".into(),
        prefix: "kb".into(),
        capabilities: vec!["read.grep".into()],
        allow: vec![],
        deny: vec![],
    };
    let skills = builtin_skills(&cx);
    assert_eq!(
        skills.iter().map(|s| s.name.as_str()).collect::<Vec<_>>(),
        [
            "okfkit-answer",
            "okfkit-curate",
            "okfkit-adopt",
            "okfkit-tune",
            "okfkit-setup"
        ]
    );
    insta::assert_snapshot!("okfkit_answer_lexical", skills[0].content);
    insta::assert_snapshot!("okfkit_curate", skills[1].content);
    insta::assert_snapshot!("okfkit_adopt", skills[2].content);
    insta::assert_snapshot!("okfkit_tune", skills[3].content);
    insta::assert_snapshot!("okfkit_setup", skills[4].content);
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
    fs::create_dir_all(project.join(".codex")).unwrap();
    fs::write(project.join(".codex/config.toml"), config).unwrap();
    fs::write(project.join("AGENTS.md"), "# Rules\n\nBe nice.\n").unwrap();

    let o = opts(
        Agent::Codex,
        Target::Project(project.clone()),
        &home,
        Path::new("/kb"),
    );
    apply(&plan(&o).unwrap()).unwrap();
    let written = fs::read_to_string(project.join(".codex/config.toml")).unwrap();
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
        fs::read_to_string(project.join(".codex/config.toml")).unwrap(),
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

fn named(mut o: InstallOptions, name: &str) -> InstallOptions {
    o.server_name = name.into();
    o.skill.prefix = default_prefix(name);
    o
}

#[test]
fn two_bundles_side_by_side_and_conflicts() {
    let tmp = tempfile::tempdir().unwrap();
    let (home, project) = (tmp.path().join("home"), tmp.path().join("proj"));
    fs::create_dir_all(&project).unwrap();
    let (a, b) = (Path::new("/kb/policies"), Path::new("/kb/docs"));
    let first = opts(Agent::Claude, Target::Project(project.clone()), &home, a);
    apply(&plan(&first).unwrap()).unwrap();
    // The default name already serves another bundle: refused, with the way out.
    let clash = plan(&opts(
        Agent::Claude,
        Target::Project(project.clone()),
        &home,
        b,
    ))
    .unwrap_err();
    let msg = clash.to_string();
    assert!(
        matches!(clash, Error::Conflict { .. })
            && msg.contains("--name")
            && msg.contains("/kb/policies"),
        "{msg}"
    );
    // A second name: own server, tool prefix and skills.
    let second = named(
        opts(Agent::Claude, Target::Project(project.clone()), &home, b),
        "okfkit-docs",
    );
    apply(&plan(&second).unwrap()).unwrap();
    let mcp: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(project.join(".mcp.json")).unwrap()).unwrap();
    assert_eq!(mcp["mcpServers"]["okfkit"]["args"][1], "/kb/policies");
    assert_eq!(mcp["mcpServers"]["okfkit-docs"]["args"][1], "/kb/docs");
    // The server runs with the prefix the skill names.
    let docs_args = mcp["mcpServers"]["okfkit-docs"]["args"].to_string();
    assert!(docs_args.ends_with(r#""--prefix","docs"]"#), "{docs_args}");
    assert!(
        !mcp["mcpServers"]["okfkit"]["args"]
            .to_string()
            .contains("--prefix")
    );
    let skill =
        fs::read_to_string(project.join(".claude/skills/okfkit-answer-docs/SKILL.md")).unwrap();
    assert!(
        skill.starts_with("---\nname: okfkit-answer-docs\n") && skill.contains("`docs_*`"),
        "{skill}"
    );
    assert!(
        project
            .join(".claude/skills/okfkit-answer/SKILL.md")
            .is_file()
    );
    // --replace switches the default name to the other bundle.
    let mut replace = opts(Agent::Claude, Target::Project(project.clone()), &home, b);
    replace.replace = true;
    assert!(plan(&replace).is_ok());
}

#[test]
fn uninstall_removes_only_okfkit() {
    let tmp = tempfile::tempdir().unwrap();
    let (home, project) = (tmp.path().join("home"), tmp.path().join("proj"));
    fs::create_dir_all(home.join(".codex")).unwrap();
    fs::create_dir_all(&project).unwrap();
    // Claude: the user's own server and skill survive; okfkit's go.
    fs::write(
        project.join(".mcp.json"),
        r#"{"mcpServers": {"other": {"command": "x"}}}"#,
    )
    .unwrap();
    fs::create_dir_all(project.join(".claude/skills/mine")).unwrap();
    fs::write(project.join(".claude/skills/mine/SKILL.md"), "mine").unwrap();
    let o = opts(
        Agent::Claude,
        Target::Project(project.clone()),
        &home,
        Path::new("/kb"),
    );
    apply(&plan(&o).unwrap()).unwrap();
    let un = UninstallOptions {
        agent: Agent::Claude,
        target: Target::Project(project.clone()),
        home: home.clone(),
        server_name: "okfkit".into(),
        skills: None,
    };
    apply(&plan_uninstall(&un).unwrap()).unwrap();
    let mcp = fs::read_to_string(project.join(".mcp.json")).unwrap();
    assert!(mcp.contains("other") && !mcp.contains("okfkit"), "{mcp}");
    assert!(!project.join(".claude/skills/okfkit-answer").exists());
    assert!(project.join(".claude/skills/mine/SKILL.md").is_file());
    assert!(
        plan_uninstall(&un).unwrap().is_empty(),
        "uninstalling twice does nothing"
    );
    // A .mcp.json that only held okfkit is deleted.
    let solo = tmp.path().join("solo");
    fs::create_dir_all(&solo).unwrap();
    apply(
        &plan(&opts(
            Agent::Claude,
            Target::Project(solo.clone()),
            &home,
            Path::new("/kb"),
        ))
        .unwrap(),
    )
    .unwrap();
    apply(
        &plan_uninstall(&UninstallOptions {
            target: Target::Project(solo.clone()),
            ..un.clone()
        })
        .unwrap(),
    )
    .unwrap();
    assert!(
        !solo.join(".mcp.json").exists() && !solo.join(".claude/skills/okfkit-answer").exists()
    );

    // Codex: config comments and other servers survive; AGENTS.md is restored.
    let config = "# mine\n[mcp_servers.other]\ncommand = \"x\"\n";
    fs::create_dir_all(project.join(".codex")).unwrap();
    fs::write(project.join(".codex/config.toml"), config).unwrap();
    fs::write(project.join("AGENTS.md"), "# Rules\n").unwrap();
    let c = opts(
        Agent::Codex,
        Target::Project(project.clone()),
        &home,
        Path::new("/kb"),
    );
    apply(&plan(&c).unwrap()).unwrap();
    apply(
        &plan_uninstall(&UninstallOptions {
            agent: Agent::Codex,
            ..un.clone()
        })
        .unwrap(),
    )
    .unwrap();
    assert_eq!(
        fs::read_to_string(project.join(".codex/config.toml")).unwrap(),
        config
    );
    assert_eq!(
        fs::read_to_string(project.join("AGENTS.md")).unwrap(),
        "# Rules\n"
    );
}

#[test]
fn codex_projects_are_independent_but_user_names_are_shared() {
    // Project installs go to <project>/.codex/config.toml: two projects, two bundles, same name.
    let tmp = tempfile::tempdir().unwrap();
    let home = tmp.path().join("home");
    let (p1, p2) = (tmp.path().join("p1"), tmp.path().join("p2"));
    fs::create_dir_all(&p1).unwrap();
    fs::create_dir_all(&p2).unwrap();
    apply(
        &plan(&opts(
            Agent::Codex,
            Target::Project(p1.clone()),
            &home,
            Path::new("/kb/one"),
        ))
        .unwrap(),
    )
    .unwrap();
    apply(
        &plan(&opts(
            Agent::Codex,
            Target::Project(p2.clone()),
            &home,
            Path::new("/kb/two"),
        ))
        .unwrap(),
    )
    .unwrap();
    assert!(
        fs::read_to_string(p1.join(".codex/config.toml"))
            .unwrap()
            .contains("/kb/one")
    );
    assert!(
        fs::read_to_string(p2.join(".codex/config.toml"))
            .unwrap()
            .contains("/kb/two")
    );
    assert!(!codex_trusts(&p1, &home));
    fs::create_dir_all(home.join(".codex")).unwrap();
    fs::write(
        home.join(".codex/config.toml"),
        format!(
            "[projects.\"{}\"]\ntrust_level = \"trusted\"\n",
            p1.display()
        ),
    )
    .unwrap();
    assert!(codex_trusts(&p1, &home));
    // User installs share one file: a second bundle needs its own name.
    apply(
        &plan(&opts(
            Agent::Codex,
            Target::User,
            &home,
            Path::new("/kb/one"),
        ))
        .unwrap(),
    )
    .unwrap();
    let err = plan(&opts(
        Agent::Codex,
        Target::User,
        &home,
        Path::new("/kb/two"),
    ))
    .unwrap_err();
    assert!(matches!(err, Error::Conflict { .. }));
    let o = named(
        opts(Agent::Codex, Target::User, &home, Path::new("/kb/two")),
        "okfkit-two",
    );
    apply(&plan(&o).unwrap()).unwrap();
    let agents = fs::read_to_string(home.join(".codex/AGENTS.md")).unwrap();
    assert!(
        agents.contains("<!-- okfkit:begin okfkit-two") && agents.contains("`two_*`"),
        "{agents}"
    );
    // Uninstalling a project leaves no empty .codex/config.toml behind.
    let un = UninstallOptions {
        agent: Agent::Codex,
        target: Target::Project(p2.clone()),
        home: home.clone(),
        server_name: "okfkit".into(),
        skills: None,
    };
    apply(&plan_uninstall(&un).unwrap()).unwrap();
    assert!(!p2.join(".codex/config.toml").exists() && !p2.join("AGENTS.md").exists());
}

#[test]
fn shared_server_install() {
    let tmp = tempfile::tempdir().unwrap();
    let (home, project) = (tmp.path().join("home"), tmp.path().join("proj"));
    fs::create_dir_all(&project).unwrap();
    let mut o = opts(
        Agent::Claude,
        Target::Project(project.clone()),
        &home,
        Path::new("https://kb.example.com/mcp"),
    );
    o.server = Server::Http {
        url: "https://kb.example.com/mcp".into(),
        token_env: Some("KB_TOKEN".into()),
    };
    o.skill.capabilities.push("remote".into());
    let actions = plan(&o).unwrap();
    apply(&actions).unwrap();
    let mcp = fs::read_to_string(project.join(".mcp.json")).unwrap();
    assert!(
        mcp.contains("\"type\": \"http\"") && mcp.contains("Bearer ${KB_TOKEN}"),
        "{mcp}"
    );
    // Only the answering skill, without the local CLI fallback.
    let skills: Vec<_> = fs::read_dir(project.join(".claude/skills"))
        .unwrap()
        .flatten()
        .map(|e| e.file_name())
        .collect();
    assert_eq!(skills, ["okfkit-answer"]);
    let skill = fs::read_to_string(project.join(".claude/skills/okfkit-answer/SKILL.md")).unwrap();
    assert!(!skill.contains("Without the MCP tools"), "{skill}");
    // Codex: url + bearer token variable.
    fs::create_dir_all(home.join(".codex")).unwrap();
    let mut c = o.clone();
    c.agent = Agent::Codex;
    apply(&plan(&c).unwrap()).unwrap();
    let cfg = fs::read_to_string(project.join(".codex/config.toml")).unwrap();
    assert!(
        cfg.contains("url = \"https://kb.example.com/mcp\"")
            && cfg.contains("bearer_token_env_var = \"KB_TOKEN\""),
        "{cfg}"
    );
}

#[test]
fn registry_records_and_checks_installs() {
    let tmp = tempfile::tempdir().unwrap();
    let (home, project, bundle) = (
        tmp.path().join("home"),
        tmp.path().join("proj"),
        tmp.path().join("kb"),
    );
    fs::create_dir_all(&project).unwrap();
    fs::create_dir_all(&bundle).unwrap();
    let bin = tmp.path().join("okfkit");
    fs::write(&bin, "").unwrap();
    let mut o = opts(
        Agent::Claude,
        Target::Project(project.clone()),
        &home,
        &bundle,
    );
    o.server = Server::Stdio {
        command: bin.clone(),
    };
    apply(&plan(&o).unwrap()).unwrap();
    let reg = tmp.path().join("cfg/installs.json");
    let rec = Install::of(&o).unwrap();
    record_install(&reg, rec.clone()).unwrap();
    record_install(&reg, rec.clone()).unwrap();
    let all = load_registry(&reg).unwrap();
    assert_eq!(all.len(), 1, "same slot recorded once");
    assert!(all[0].skills.contains(&"okfkit-answer".to_owned()));
    assert!(all[0].check(&home).is_empty(), "{:?}", all[0].check(&home));
    // The bundle moved and the binary is gone: both reported.
    fs::remove_dir_all(&bundle).unwrap();
    fs::remove_file(&bin).unwrap();
    let problems = all[0].check(&home).join("; ");
    assert!(
        problems.contains("bundle") && problems.contains("binary"),
        "{problems}"
    );
    forget_install(&reg, Agent::Claude, &Target::Project(project), "okfkit").unwrap();
    assert!(load_registry(&reg).unwrap().is_empty());
}
