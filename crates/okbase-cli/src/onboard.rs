//! `okbase onboard`: a setup plan for agents, computed from the real state of the machine and
//! the bundle (docs/plans/onboarding.md §3.1). Run it, do the first step that is not done, run it
//! again; stop at every `ask` step and wait for the user's answer.

use serde::Serialize;

use okbase::advise::{Advice, Tier, When};

/// What a step asks the agent to do.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Kind {
    /// Run the command(s); no decision needed.
    Run,
    /// Ask the user `question`; run the commands of the chosen option.
    Ask,
    /// Tell the user something (the agent cannot do it).
    Tell,
}

/// One answer to an `ask` step.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Choice {
    /// The answer.
    pub answer: String,
    /// Commands to run for it (empty: nothing).
    pub commands: Vec<String>,
}

/// A step of the plan.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Step {
    /// Stable id.
    pub id: &'static str,
    /// Run, ask or tell.
    pub kind: Kind,
    /// What it does.
    pub title: String,
    /// Why (with the measurement behind it).
    pub why: String,
    /// For `run`: the commands.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub commands: Vec<String>,
    /// For `ask` / `tell`: what to say to the user.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub question: Option<String>,
    /// For `ask`: the answers.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub options: Vec<Choice>,
    /// Files or places this changes.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub writes: Vec<String>,
}

/// What onboarding checks.
#[derive(Debug, Clone, Default)]
pub struct State {
    /// okbase version.
    pub version: String,
    /// This build has local embeddings / fine-tuning.
    pub full_build: bool,
    /// The bundle directory exists.
    pub bundle_exists: bool,
    /// Bundle path as given.
    pub bundle: String,
    /// The advice, when the bundle exists.
    pub advice: Option<Advice>,
    /// Agents found on this machine (labels).
    pub agents_found: Vec<&'static str>,
    /// okbase runs inside a Codex session, whose sandbox keeps `.codex/` read-only.
    pub inside_codex: bool,
    /// Healthy installs serving this bundle (agent labels).
    pub installed_for: Vec<&'static str>,
    /// Problems with installs serving this bundle.
    pub install_problems: Vec<String>,
    /// EmbeddingGemma's license is accepted.
    pub gemma_accepted: bool,
    /// The bundle has a tune run.
    pub tune_started: bool,
    /// What the folder is (`okbase scan`).
    pub scan: Option<okbase::scan::Scan>,
    /// Source documents read directly (PDF, Word…).
    pub sources: usize,
    /// Source documents with pages that have no text: (path, pages).
    pub pending_ocr: Vec<(String, usize)>,
}

/// Goals that shorten the plan.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Goal {
    /// Everything `advise` recommends.
    #[default]
    Answer,
    /// Only organizing the bundle.
    Curate,
    /// Remove okbase.
    Remove,
}

/// The plan.
#[derive(Debug, Clone, Serialize)]
pub struct Plan {
    /// Bundle.
    pub bundle: String,
    /// One-line summary.
    pub summary: String,
    /// What is already in place.
    pub done: Vec<String>,
    /// What to do, in order.
    pub steps: Vec<Step>,
    /// Rules every agent follows.
    pub rules: Vec<&'static str>,
}

/// Rules repeated in every plan.
pub const RULES: &[&str] = &[
    "Do the steps in order. Run `okbase onboard` again after each step: finished steps move to `done`.",
    "At an `ask` step, ask the user the question and wait. Run only the commands of the option they chose.",
    "Never add --accept-license, --yes, --write, --force, --replace or --send-documents unless the user agreed to that step.",
    "Never edit documents without asking. A command that exits 3 needs the user's consent: relay its `question`.",
];

fn run(id: &'static str, title: &str, why: &str, commands: &[&str], writes: &[&str]) -> Step {
    Step {
        id,
        kind: Kind::Run,
        title: title.into(),
        why: why.into(),
        commands: commands.iter().map(|c| (*c).to_owned()).collect(),
        question: None,
        options: Vec::new(),
        writes: writes.iter().map(|c| (*c).to_owned()).collect(),
    }
}

fn ask(
    id: &'static str,
    title: &str,
    why: &str,
    question: String,
    options: Vec<Choice>,
    writes: &[&str],
) -> Step {
    Step {
        id,
        kind: Kind::Ask,
        title: title.into(),
        why: why.into(),
        commands: Vec::new(),
        question: Some(question),
        options,
        writes: writes.iter().map(|c| (*c).to_owned()).collect(),
    }
}

fn choice(answer: &str, commands: &[&str]) -> Choice {
    Choice {
        answer: answer.into(),
        commands: commands.iter().map(|c| (*c).to_owned()).collect(),
    }
}

fn tell(id: &'static str, title: &str, why: &str, say: String) -> Step {
    Step {
        id,
        kind: Kind::Tell,
        title: title.into(),
        why: why.into(),
        commands: Vec::new(),
        question: Some(say),
        options: Vec::new(),
        writes: Vec::new(),
    }
}

/// Computes the plan.
pub fn plan(st: &State, goal: Goal) -> Plan {
    let mut done = vec![format!(
        "okbase {} ({})",
        st.version,
        if st.full_build {
            "okbase-full: embeddings, fine-tuning"
        } else {
            "default build: lexical tools"
        }
    )];
    let mut steps = Vec::new();
    let summary;
    if goal == Goal::Remove {
        summary = "remove okbase from this machine".to_owned();
        steps.push(ask(
            "remove-agents",
            "Unregister okbase from the agents",
            "removes only okbase's MCP entries, skills and AGENTS.md blocks, everywhere it was installed",
            "Remove okbase from every agent configuration where it was installed (other settings stay)?".into(),
            vec![choice("yes", &["okbase agent uninstall --all"]), choice("no", &[])],
            &["agent configurations"],
        ));
        steps.push(ask(
            "remove-data",
            "Delete okbase's index and cache",
            "models, vectors, dictionary and training environment; documents are never touched",
            "Delete okbase's downloaded models, caches and indexes (`okbase clean` shows the sizes)?".into(),
            vec![choice("yes", &["okbase clean --all --yes"]), choice("no", &[])],
            &["user cache"],
        ));
        steps.push(tell(
            "remove-binary",
            "Remove the program",
            "the agent should not uninstall software on its own",
            "To remove the program itself, run: cargo uninstall okbase-cli".into(),
        ));
        return Plan {
            bundle: st.bundle.clone(),
            summary,
            done,
            steps,
            rules: RULES.to_vec(),
        };
    }
    if let Some(sc) = st.scan.as_ref().filter(|_| st.bundle_exists) {
        use okbase::scan::FolderKind;
        if sc.kind == FolderKind::Empty {
            {
                steps.push(ask(
                    "init",
                    "Start a knowledge base here",
                    "the folder is empty: okbase needs markdown documents to work with",
                    "This folder is empty. Shall I start a knowledge base here? Tell me what it is about, who will use it, and which languages people ask in; if you already have documents elsewhere (markdown, PDF, Word), tell me where.".into(),
                    vec![
                        choice("yes", &["okbase init --title \"<what it is about>\" --langs <vi,en,…>", "okbase new --type <Type> \"<title>\"   (or follow the okbase-author skill)", "okbase onboard"]),
                        choice("my documents are elsewhere", &["okbase -b <that folder> onboard"]),
                    ],
                    &["index.md, _meta/ and okbase.toml in this folder"],
                ));
                return Plan {
                    bundle: st.bundle.clone(),
                    summary: "empty folder".into(),
                    done,
                    steps,
                    rules: RULES.to_vec(),
                };
            }
        }
        let elsewhere = match sc.recommended.map(|i| &sc.candidates[i]) {
            Some(c) if c.path != "." => Some(vec![c]),
            None if sc.candidates.len() > 1 => Some(sc.candidates.iter().collect()),
            _ => None,
        };
        if let Some(cands) = elsewhere {
            let describe = |c: &okbase::scan::Candidate| {
                format!(
                    "{} ({} docs, {:?}, level {})",
                    c.path,
                    c.docs,
                    c.kind,
                    c.level.map_or("below L0".into(), |l| l.to_string())
                )
            };
            if let [c] = cands.as_slice() {
                steps.push(run(
                    "bundle",
                    &format!("Use {} as the knowledge bundle", c.path),
                    &format!(
                        "{}; the rest of the folder ({}) is not knowledge",
                        describe(c),
                        sc.signals.join(", ")
                    ),
                    &[&format!("okbase -b {} onboard", c.path)],
                    &[],
                ));
            } else {
                steps.push(ask(
                    "bundle",
                    "Choose the knowledge folder",
                    "several folders could be the bundle",
                    format!(
                        "Which folder should agents answer from? {}",
                        cands
                            .iter()
                            .map(|c| describe(c))
                            .collect::<Vec<_>>()
                            .join("; ")
                    ),
                    cands
                        .iter()
                        .map(|c| Choice {
                            answer: c.path.clone(),
                            commands: vec![format!("okbase -b {} onboard", c.path)],
                        })
                        .collect(),
                    &[],
                ));
            }
            return Plan {
                bundle: st.bundle.clone(),
                summary: format!("{:?} folder; the bundle is a subfolder", sc.kind).to_lowercase(),
                done,
                steps,
                rules: RULES.to_vec(),
            };
        }
    }
    let here = st
        .scan
        .as_ref()
        .and_then(|sc| sc.candidates.iter().find(|c| c.path == "."));
    let Some(advice) = st.advice.as_ref().filter(|_| st.bundle_exists) else {
        steps.push(ask(
            "bundle",
            "Find the knowledge folder",
            "okbase works on a folder of markdown files",
            format!(
                "{} is not a folder. Which folder holds the documents the agent should answer from?",
                st.bundle
            ),
            vec![choice("a folder path", &["okbase -b <folder> onboard"])],
            &[],
        ));
        return Plan {
            bundle: st.bundle.clone(),
            summary: "no bundle yet".into(),
            done,
            steps,
            rules: RULES.to_vec(),
        };
    };
    let p = &advice.profile;
    let mut langs: Vec<_> = p.langs.iter().collect();
    langs.sort_by(|a, b| b.1.total_cmp(a.1));
    summary = format!(
        "{} docs, ~{} tokens, {}, level {}",
        p.docs,
        p.tokens,
        langs
            .iter()
            .map(|(k, v)| format!("{k} {:.0}%", *v * 100.0))
            .collect::<Vec<_>>()
            .join(" / "),
        p.level.map_or("below L0".into(), |l| l.to_string())
    );
    done.push("index up to date".into());

    let connect = advice
        .steps
        .iter()
        .find(|s| matches!(s.tier, Tier::Full | Tier::Lexical));
    if goal == Goal::Answer {
        if !st.install_problems.is_empty() {
            steps.push(ask(
                "agents-repair",
                "Repair the agent setup",
                &st.install_problems.join("; "),
                format!(
                    "The okbase setup for this bundle is broken ({}). Repair it by pointing the agents at {}?",
                    st.install_problems.join("; "),
                    st.bundle
                ),
                vec![choice("yes", &["okbase agent install --replace"]), choice("no", &[])],
                &[".mcp.json", ".claude/skills", ".codex/config.toml", "AGENTS.md"],
            ));
        } else if st.installed_for.is_empty() {
            if st.agents_found.is_empty() {
                steps.push(tell(
                    "agents",
                    "No agent CLI found",
                    "okbase registers itself with Claude Code or Codex",
                    "Install Claude Code or Codex, then run `okbase onboard` again; or use `okbase mcp serve` with another MCP client.".into(),
                ));
            } else {
                let why =
                    connect.map_or("agents get the okbase tools and skills", |c| c.why.as_str());
                // Inside Codex, its sandbox refuses writes to .codex/: the user connects Codex.
                let sandboxed = st.inside_codex && st.agents_found.contains(&"Codex");
                let here: Vec<&str> = st
                    .agents_found
                    .iter()
                    .copied()
                    .filter(|a| !(sandboxed && *a == "Codex"))
                    .collect();
                if !here.is_empty() {
                    let (command, writes): (&str, &[&str]) = if sandboxed {
                        (
                            "okbase agent install --claude",
                            &[".mcp.json", ".claude/skills/", "AGENTS.md"],
                        )
                    } else {
                        (
                            "okbase agent install",
                            &[
                                ".mcp.json",
                                ".claude/skills/",
                                ".codex/config.toml",
                                "AGENTS.md",
                            ],
                        )
                    };
                    let mut s = run(
                        "agents",
                        &format!("Connect {} to the bundle", here.join(" and ")),
                        why,
                        &[command],
                        writes,
                    );
                    s.question = Some(
                        "Tell the user what was written, then: restart the agent session (or reconnect MCP) to load the okbase tools.".into(),
                    );
                    steps.push(s);
                }
                if sandboxed {
                    steps.push(tell(
                        "agents-codex",
                        "Connect Codex (the user runs one command)",
                        "Codex's sandbox keeps .codex/ read-only, so okbase cannot connect Codex from inside Codex",
                        "Please run `okbase agent install --codex` in your own terminal in this folder (add `--user` for every project), then restart Codex. Until then I can use the okbase CLI.".into(),
                    ));
                }
            }
        } else {
            done.push(format!("connected: {}", st.installed_for.join(", ")));
        }
        if connect.is_some_and(|c| c.tier == Tier::Full) {
            done.push(
                "small bundle: it fits in the agent's context; `okbase catalog` lists it".into(),
            );
        }
    }

    for s in &advice.steps {
        match s.tier {
            Tier::Curate
                if here.is_some_and(|c| !c.to_fix.is_empty() && c.conformant * 5 >= c.docs) =>
            {
                // Mostly OKF: fix the few files in place.
                let c = here.expect("checked");
                let only: Vec<String> = c.to_fix.iter().map(|f| format!("--only {f}")).collect();
                steps.push(ask(
                    "fix",
                    &format!("Fix {} of {} documents", c.docs - c.conformant, c.docs),
                    &format!("{} of {} documents already have OKF frontmatter; the rest block the level", c.conformant, c.docs),
                    format!(
                        "{} documents need frontmatter fixes ({}{}). May I fix them in place? (review with git diff)",
                        c.docs - c.conformant,
                        c.to_fix.iter().take(5).cloned().collect::<Vec<_>>().join(", "),
                        if c.to_fix.len() > 5 { ", …" } else { "" }
                    ),
                    vec![
                        Choice {
                            answer: "yes".into(),
                            commands: vec![
                                format!("okbase adopt {} --write", only.join(" ")),
                                "okbase lint --level L1 --json   (fix what remains by hand: invalid YAML is reported with its line)".into(),
                            ],
                        },
                        choice("no", &[]),
                    ],
                    &["the listed documents"],
                ));
            }
            Tier::Curate
                if here.is_some_and(|c| {
                    matches!(
                        c.kind,
                        okbase::scan::CandidateKind::DocsSite | okbase::scan::CandidateKind::Vault
                    )
                }) && p.level.is_none() =>
            {
                steps.push(ask(
                    "metadata",
                    "Add titles and descriptions (optional)",
                    "the documents also build a site or a vault; okbase already works on them as they are, and descriptions help agents pick the right page",
                    "okbase can use these documents as they are. Shall I also add a title and a one-line description to the frontmatter of each page, in place? The site keeps working (index.md is kept); review with git diff.".into(),
                    vec![
                        choice("yes", &["okbase adopt --write", "follow the okbase-curate skill to rewrite the generated descriptions"]),
                        choice("no, use them as they are", &[]),
                    ],
                    &["frontmatter of the documents"],
                ));
            }
            Tier::Curate => {
                let adopt = p.level.is_none();
                steps.push(ask(
                    "curate",
                    &s.title,
                    &s.why,
                    if adopt {
                        "The folder is plain markdown. May I create an OKF copy of it with titles, descriptions and index.md (the original is not touched)?".into()
                    } else {
                        format!(
                            "{} May I improve descriptions, index.md and tags in the documents? (files change; review them with git diff)",
                            if p.missing_descriptions > 0 {
                                format!("{} documents have no description.", p.missing_descriptions)
                            } else {
                                "The bundle is below level L2.".into()
                            }
                        )
                    },
                    if adopt {
                        vec![
                            Choice {
                                answer: "yes".into(),
                                commands: vec![
                                    format!("okbase adopt {} --out <new folder>", st.bundle),
                                    "okbase -b <new folder> onboard".into(),
                                ],
                            },
                            choice("no", &[]),
                        ]
                    } else {
                        vec![
                            choice("yes", &["okbase lint --level L2 --json", "follow the okbase-curate skill"]),
                            choice("no", &[]),
                        ]
                    },
                    &["documents in the bundle"],
                ));
            }
            Tier::Data if goal == Goal::Answer => {
                done.push("tables found: SQL tools (data_tables, data_query) are on".into());
            }
            Tier::Embed if goal == Goal::Answer => {
                if s.when == When::Done {
                    done.push(s.title.clone());
                } else if !st.full_build {
                    steps.push(ask(
                        "embed-build",
                        "Semantic search needs the okbase-full build",
                        &s.why,
                        "Semantic search needs the okbase-full build of okbase (about 60 MB instead of 34 MB, plus a model download). Install it?".into(),
                        vec![
                            choice("yes", &[crate::contract::INSTALL_FULL, "okbase onboard"]),
                            choice("no", &[]),
                        ],
                        &["the okbase program"],
                    ));
                } else if p.embed.is_some() {
                    steps.push(run(
                        "embed-index",
                        "Embed the bundle",
                        &s.why,
                        &["okbase embed index"],
                        &["user cache (vectors)"],
                    ));
                } else {
                    let gemma = if st.gemma_accepted {
                        "okbase embed enable"
                    } else {
                        "okbase embed enable --accept-license"
                    };
                    steps.push(ask(
                        "embed",
                        &s.title,
                        &s.why,
                        format!(
                            "{}Semantic search downloads a model once: EmbeddingGemma 300M (188 MB, Gemma Terms of Use: https://ai.google.dev/gemma/terms; best for questions across languages) or bge-m3 (570 MB, MIT). Which one, or neither?",
                            if s.when == When::Maybe { "Optional. " } else { "" }
                        ),
                        vec![
                            choice("EmbeddingGemma (accept the Gemma terms)", &[gemma, "okbase embed index"]),
                            choice("bge-m3 (MIT)", &["okbase embed enable --model bge-m3-int8", "okbase embed index"]),
                            choice("neither", &[]),
                        ],
                        &["okbase.toml", "user cache (model, vectors)"],
                    ));
                }
            }
            Tier::Tune if goal == Goal::Answer => {
                if st.tune_started {
                    done.push("fine-tuning started: `okbase embed tune status`".into());
                } else {
                    steps.push(ask(
                        "tune",
                        &s.title,
                        &s.why,
                        "Optional, later: fine-tune the search model on this bundle? I would write practice questions about the documents (their text is sent to my model provider), then train for ~10–25 minutes on this machine (~1.7 GB download).".into(),
                        vec![
                            choice("yes", &["follow the okbase-tune skill (or: okbase embed tune guide)"]),
                            choice("not now", &[]),
                        ],
                        &["user cache"],
                    ));
                }
            }
            _ => {}
        }
    }
    if goal == Goal::Answer && st.sources > 0 {
        done.push(format!(
            "{} source documents (PDF, Word, PowerPoint, HTML…) are searchable as they are",
            st.sources
        ));
    }
    if goal == Goal::Answer && !st.pending_ocr.is_empty() {
        let pages: usize = st.pending_ocr.iter().map(|(_, n)| n).sum();
        steps.push(ask(
            "ocr",
            &format!("Transcribe {pages} pages that have no text"),
            "scanned pages and images have no extractable text, so nothing on them can be found; okbase has no OCR engine and never sends files to an OCR service",
            format!(
                "{} documents have {pages} scanned pages or images without text ({}). May I read those pages and transcribe them? Their content goes to my model provider; nothing in the folder changes.",
                st.pending_ocr.len(),
                st.pending_ocr.iter().take(3).map(|(p, _)| p.as_str()).collect::<Vec<_>>().join(", ")
            ),
            vec![
                choice("yes", &["okbase import ocr-next   (repeat until nothing is left; follow the okbase-import skill)"]),
                choice("no", &[]),
            ],
            &["okbase's state directory only"],
        ));
    }
    if !steps.is_empty() {
        steps.push(run(
            "verify",
            "Check the setup",
            "proves the MCP server starts and answers before the user restarts the agent",
            &["okbase doctor"],
            &[],
        ));
    }
    Plan {
        bundle: st.bundle.clone(),
        summary,
        done,
        steps,
        rules: RULES.to_vec(),
    }
}

impl Plan {
    /// Text for agents and people.
    pub fn to_text(&self) -> String {
        let mut out = format!(
            "okbase onboarding: {} ({})\n\nDone\n",
            self.bundle, self.summary
        );
        for d in &self.done {
            out.push_str(&format!("  ✓ {d}\n"));
        }
        if self.steps.is_empty() {
            out.push_str("\nNothing left to do. `okbase doctor` checks the setup at any time.\n");
        } else {
            out.push_str("\nTo do (in order; stop at every ASK and wait for the user's answer)\n");
        }
        for (i, s) in self.steps.iter().enumerate() {
            let tag = match s.kind {
                Kind::Run => "run",
                Kind::Ask => "ASK",
                Kind::Tell => "tell",
            };
            out.push_str(&format!(
                "  {}. [{tag}] {}\n       why: {}\n",
                i + 1,
                s.title,
                s.why
            ));
            for c in &s.commands {
                out.push_str(&format!("       $ {c}\n"));
            }
            if let Some(q) = &s.question {
                out.push_str(&format!("       \"{q}\"\n"));
            }
            for o in &s.options {
                out.push_str(&format!(
                    "       {} → {}\n",
                    o.answer,
                    if o.commands.is_empty() {
                        "skip".into()
                    } else {
                        o.commands.join(" && ")
                    }
                ));
            }
            if !s.writes.is_empty() {
                out.push_str(&format!("       changes: {}\n", s.writes.join(", ")));
            }
        }
        out.push_str("\nRules for agents\n");
        for r in &self.rules {
            out.push_str(&format!("  - {r}\n"));
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use okbase::advise::{AdviseOptions, Profile};

    fn advice(
        docs: usize,
        tokens: usize,
        langs: &[(&str, f64)],
        level: Option<okbase::Level>,
    ) -> Advice {
        let profile = Profile {
            docs,
            tokens,
            chunks: docs * 2,
            langs: langs.iter().map(|(k, v)| ((*k).into(), *v)).collect(),
            level,
            missing_descriptions: 0,
            has_index: true,
            datasets: false,
            embed: None,
            build: okbase::build_features(),
        };
        okbase::advise::plan_for(
            profile,
            &AdviseOptions {
                user_langs: vec!["vi".into()],
                ..Default::default()
            },
        )
    }

    fn state(a: Advice) -> State {
        State {
            version: "0.3.0".into(),
            full_build: true,
            bundle_exists: true,
            bundle: "/kb".into(),
            advice: Some(a),
            agents_found: vec!["Claude Code"],
            ..Default::default()
        }
    }

    fn ids(p: &Plan) -> Vec<&str> {
        p.steps.iter().map(|s| s.id).collect()
    }

    #[test]
    fn fresh_cross_language_bundle() {
        let p = plan(
            &state(advice(
                287,
                1_100_000,
                &[("en", 1.0)],
                Some(okbase::Level::L2),
            )),
            Goal::Answer,
        );
        assert_eq!(ids(&p), ["agents", "embed", "tune", "verify"]);
        let embed = &p.steps[1];
        assert_eq!(embed.kind, Kind::Ask);
        assert!(embed.options[0].commands[0].contains("--accept-license"));
        assert!(
            embed
                .question
                .as_ref()
                .unwrap()
                .contains("Gemma Terms of Use")
        );
    }

    #[test]
    fn finished_steps_disappear() {
        let mut st = state(advice(
            287,
            1_100_000,
            &[("en", 1.0)],
            Some(okbase::Level::L2),
        ));
        st.installed_for = vec!["Claude Code"];
        st.gemma_accepted = true;
        st.tune_started = true;
        let p = plan(&st, Goal::Answer);
        assert_eq!(ids(&p), ["embed", "verify"]);
        assert!(
            p.steps[0].options[0].commands[0] == "okbase embed enable",
            "no license flag once accepted"
        );
        assert!(p.done.iter().any(|d| d.starts_with("connected")));
    }

    #[test]
    fn small_bundle_and_repair_and_remove() {
        let mut st = state(advice(20, 10_000, &[("en", 1.0)], Some(okbase::Level::L2)));
        st.installed_for = vec!["Claude Code"];
        assert!(plan(&st, Goal::Answer).steps.is_empty(), "nothing to do");
        st.install_problems = vec!["the bundle /old no longer exists".into()];
        let p = plan(&st, Goal::Answer);
        assert_eq!(p.steps[0].id, "agents-repair");
        assert_eq!(p.steps[0].kind, Kind::Ask, "--replace needs consent");
        let r = plan(&st, Goal::Remove);
        assert_eq!(ids(&r), ["remove-agents", "remove-data", "remove-binary"]);
        assert!(r.to_text().contains("[ASK]"));
    }

    #[test]
    fn no_bundle_and_plain_markdown() {
        let st = State {
            bundle: "/nope".into(),
            version: "0.3.0".into(),
            ..Default::default()
        };
        assert_eq!(ids(&plan(&st, Goal::Answer)), ["bundle"]);
        let p = plan(
            &state(advice(200, 300_000, &[("en", 1.0)], None)),
            Goal::Curate,
        );
        assert_eq!(ids(&p), ["curate", "verify"]);
        assert!(p.steps[0].options[0].commands[0].starts_with("okbase adopt /kb --out"));
    }
}
