//! `advise`: how to use okbase for a bundle, from the simplest setup up (docs/plans/advise-tune.md §1).
//!
//! Lexical and model-free: it reads the index, the lint report and `okbase.toml`, and only
//! prints steps. Every threshold below cites the spike it comes from.

use std::collections::BTreeMap;

use serde::Serialize;

use crate::{BuildFeatures, Bundle, Error, Level, LintConfig, Scope};

/// A language counts as part of the bundle from this share of its tokens.
pub const LANG_MIN_SHARE: f64 = 0.05;
/// Above this many tokens, embeddings are worth considering even for one language: the
/// okf-scale spike (S4) found lexical search 90–100% accurate at every size, but embeddings
/// cut agent turns (~2 vs ~4.5) on the large bundles.
pub const EMBED_LARGE_TOKENS: usize = 1_000_000;
/// Fine-tuning needs enough documents to generate ~300+ training pairs and a held-out set (S11).
pub const TUNE_MIN_DOCS: usize = 100;
/// Share of concepts without a description above which curation comes first.
pub const MISSING_DESCRIPTION_MAX_SHARE: f64 = 0.1;

/// Who will use the bundle.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Audience {
    /// Claude Code on this machine.
    Claude,
    /// OpenAI Codex CLI on this machine.
    Codex,
    /// A team sharing one server.
    Team,
    /// An application embedding okbase (library or MCP router).
    Host,
}

/// Inputs to [`Bundle::advise`] that the bundle cannot tell.
#[derive(Debug, Clone, Default)]
pub struct AdviseOptions {
    /// Languages people ask in (ISO 639-1); empty means the bundle's own languages.
    pub user_langs: Vec<String>,
    /// Who will use the bundle; `None` shows the commands for Claude Code and Codex.
    pub audience: Option<Audience>,
    /// Documents must not be sent to a cloud LLM (affects fine-tuning).
    pub private: bool,
}

/// One rung of the ladder, simplest first.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Tier {
    /// The whole bundle in the agent's context.
    Full,
    /// Catalog plus grep/query/get tools (the default).
    Lexical,
    /// Adopt or curate the bundle (descriptions, index.md, tags).
    Curate,
    /// SQL over CSV/TSV/XLSX.
    Data,
    /// Semantic search (module embed).
    Embed,
    /// Fine-tune the embedding model on the bundle.
    Tune,
}

/// When to take a step.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum When {
    /// Do this now.
    Now,
    /// Worth doing after the `now` steps.
    Next,
    /// Only if measurements show a need.
    Maybe,
    /// Already set up.
    Done,
}

/// A recommended step.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Step {
    /// Rung.
    pub tier: Tier,
    /// When.
    pub when: When,
    /// What to do, in a few words.
    pub title: String,
    /// Why, citing the measurement behind it.
    pub why: String,
    /// Commands to run, in order.
    pub commands: Vec<String>,
}

/// A rung that is not needed, and why.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Skipped {
    /// Rung.
    pub tier: Tier,
    /// Why not.
    pub why: String,
}

/// What `advise` measured.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Profile {
    /// Visible concepts.
    pub docs: usize,
    /// Estimated tokens.
    pub tokens: usize,
    /// Chunks.
    pub chunks: usize,
    /// Share of tokens per language (`lang` field, else detected).
    pub langs: BTreeMap<String, f64>,
    /// Highest okbase level met.
    pub level: Option<Level>,
    /// Concepts without a description.
    pub missing_descriptions: usize,
    /// Whether the bundle has a root `index.md`.
    pub has_index: bool,
    /// Whether the bundle has CSV/TSV/XLSX files.
    pub datasets: bool,
    /// Configured embedding model and progress.
    pub embed: Option<crate::EmbedStatus>,
    /// Optional modules in this build.
    pub build: BuildFeatures,
}

/// The advice for a bundle.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Advice {
    /// Measurements.
    pub profile: Profile,
    /// Recommended steps, simplest first.
    pub steps: Vec<Step>,
    /// Rungs that are not needed.
    pub skipped: Vec<Skipped>,
}

impl Bundle {
    /// Recommends how to use okbase for this bundle (read-only; call [`Bundle::sync`] first).
    pub fn advise(&self, options: &AdviseOptions, scope: &Scope) -> Result<Advice, Error> {
        let stats = self.stats(scope)?;
        let lang_tokens = okbase_query::content_langs(&self.index(), scope)?;
        let lint = self.lint(&LintConfig::level(Level::L2))?;
        let missing_descriptions = lint
            .diagnostics
            .iter()
            .filter(|d| d.rule == "missing-description")
            .count();
        let total = lang_tokens.values().sum::<usize>().max(1) as f64;
        let langs = lang_tokens
            .iter()
            .map(|(k, v)| (k.clone(), (*v as f64 / total * 1000.0).round() / 1000.0))
            .collect();
        let profile = Profile {
            docs: stats.docs,
            tokens: stats.tokens,
            chunks: stats.chunks,
            langs,
            level: lint.level,
            missing_descriptions,
            has_index: self.root().join("index.md").is_file(),
            datasets: okbase_data::Data::has_datasets(self.root()),
            embed: self.embed_status().ok(),
            build: crate::build_features(),
        };
        Ok(plan(profile, options))
    }
}

/// Languages that make up at least [`LANG_MIN_SHARE`] of the bundle.
fn main_langs(p: &Profile) -> Vec<String> {
    p.langs
        .iter()
        .filter(|(k, v)| **v >= LANG_MIN_SHARE && k.as_str() != "other")
        .map(|(k, _)| k.clone())
        .collect()
}

/// The advice for a measured profile (what [`Bundle::advise`] returns after measuring).
pub fn plan_for(profile: Profile, o: &AdviseOptions) -> Advice {
    plan(profile, o)
}

fn plan(profile: Profile, o: &AdviseOptions) -> Advice {
    let mut steps = Vec::new();
    let mut skipped = Vec::new();
    let mut skip = |tier, why: String| skipped.push(Skipped { tier, why });
    let p = &profile;

    let doc_langs = main_langs(p);
    let user_langs: Vec<String> = if o.user_langs.is_empty() {
        doc_langs.clone()
    } else {
        o.user_langs.iter().map(|l| l.to_lowercase()).collect()
    };
    let foreign: Vec<&String> = user_langs
        .iter()
        .filter(|l| !doc_langs.contains(l))
        .collect();
    // Cross-language questions: people ask in a language the documents are not (mostly) in,
    // or the bundle mixes languages so a question in one must find documents in another.
    let cross = !foreign.is_empty() || doc_langs.len() > 1;
    let fits = p.tokens <= okbase_query::FULL_MODE_MAX_TOKENS;

    // 0/1. Full context or lexical tools.
    let install = match o.audience {
        Some(Audience::Claude) => vec!["okbase agent install --claude".to_owned()],
        Some(Audience::Codex) => vec!["okbase agent install --codex".to_owned()],
        Some(Audience::Team) => vec![
            "OKBASE_MCP_TOKEN=<secret> okbase mcp serve --http 0.0.0.0:7331 --allow-host <host>"
                .to_owned(),
            "each member: okbase agent install --url https://<host>/mcp --token-env <VAR>"
                .to_owned(),
        ],
        Some(Audience::Host) => vec![
            "use okbase::Bundle in-process, or mount okbase_mcp::router() (MCP over HTTP)"
                .to_owned(),
        ],
        None => vec![
            "okbase agent install   # Claude Code and/or Codex, whichever is installed".to_owned(),
        ],
    };
    if fits {
        steps.push(Step {
            tier: Tier::Full,
            when: When::Now,
            title: "Put the whole bundle in the agent's context".into(),
            why: format!(
                "~{} tokens fits in context (≤ {}k): full context was fastest and cheapest in the okf-scale spike",
                p.tokens,
                okbase_query::FULL_MODE_MAX_TOKENS / 1000
            ),
            commands: std::iter::once("okbase catalog".to_owned())
                .chain(install)
                .collect(),
        });
    } else {
        steps.push(Step {
            tier: Tier::Lexical,
            when: When::Now,
            title: "Give the agent the catalog and lexical tools".into(),
            why: format!(
                "~{} tokens is too large for context; lexical tools reached 90–100% in the okf-scale spike (S4) with no model",
                p.tokens
            ),
            commands: install,
        });
    }

    // 1+. Organization first.
    let missing_share = p.missing_descriptions as f64 / p.docs.max(1) as f64;
    match p.level {
        None => steps.push(Step {
            tier: Tier::Curate,
            when: When::Now,
            title: "Adopt the folder into the okbase standard".into(),
            why: "the files are not OKF-conformant yet; a well-organized bundle is what makes agents accurate (spikes S3-S5, spikes/README.md)".into(),
            commands: vec![
                "okbase adopt --plan".into(),
                "okbase adopt --out <dir>   # or --write after review".into(),
            ],
        }),
        Some(level) if level < Level::L2 || missing_share > MISSING_DESCRIPTION_MAX_SHARE || !p.has_index => {
            let mut why = vec![format!("level {level}")];
            if p.missing_descriptions > 0 {
                why.push(format!("{} documents without a description", p.missing_descriptions));
            }
            if !p.has_index {
                why.push("no index.md".into());
            }
            steps.push(Step {
                tier: Tier::Curate,
                when: When::Now,
                title: "Curate: descriptions, index.md, tags".into(),
                why: format!(
                    "{}; agents rely on descriptions and index.md to pick documents (spikes S3-S5, spikes/README.md)",
                    why.join(", ")
                ),
                commands: vec![
                    "okbase lint --level L2".into(),
                    "ask your agent to follow the okbase-curate skill".into(),
                ],
            });
        }
        Some(level) => skip(Tier::Curate, format!("level {level} met, index.md present")),
    }

    // 2. Tables.
    if p.datasets {
        steps.push(Step {
            tier: Tier::Data,
            when: When::Now,
            title: "Query tables with SQL (on automatically)".into(),
            why: "the bundle has CSV/TSV/XLSX files; without SQL agents gave up on large sheets, with it 10/10 at 9× lower cost (S5)".into(),
            commands: vec!["okbase data tables".into()],
        });
    } else {
        skip(Tier::Data, "no CSV/TSV/XLSX files".into());
    }

    // 3. Embeddings.
    let embed_build = p.build.embed_local || p.build.embed_api;
    let needs_build = |mut c: Vec<String>| {
        if !embed_build {
            c.insert(
                0,
                "install the okbase-full build (this one has no embed module)".into(),
            );
        }
        c
    };
    let enable = || {
        needs_build(vec![
            "okbase embed enable --accept-license   # EmbeddingGemma 300M Q4 (Gemma terms); MIT: --model bge-m3-int8".into(),
            "okbase embed index".into(),
        ])
    };
    let embedded = p
        .embed
        .as_ref()
        .is_some_and(|e| e.chunks > 0 && e.complete());
    if fits {
        skip(Tier::Embed, "the bundle fits in context".into());
    } else if let Some(e) = p.embed.as_ref() {
        steps.push(Step {
            tier: Tier::Embed,
            when: if embedded { When::Done } else { When::Now },
            title: format!("Semantic search with {}", e.model),
            why: format!("{}/{} chunks embedded", e.embedded, e.chunks),
            commands: if embedded {
                vec![]
            } else {
                vec!["okbase embed index".into()]
            },
        });
    } else if cross {
        let why = if foreign.is_empty() {
            format!(
                "the bundle mixes {}: a question in one language must find documents in another, and lexical search cannot match across languages (BM25 3.5% in S1; embeddings 0.85 R@1)",
                doc_langs.join("/")
            )
        } else {
            format!(
                "people ask in {} but the documents are in {}: lexical search cannot match across languages (BM25 3.5% in S1; embeddings 0.85 R@1)",
                foreign
                    .iter()
                    .map(|s| s.as_str())
                    .collect::<Vec<_>>()
                    .join("/"),
                if doc_langs.is_empty() {
                    "other languages".to_owned()
                } else {
                    doc_langs.join("/")
                }
            )
        };
        steps.push(Step {
            tier: Tier::Embed,
            when: When::Next,
            title: "Add semantic search".into(),
            why,
            commands: enable(),
        });
    } else if p.tokens > EMBED_LARGE_TOKENS {
        steps.push(Step {
            tier: Tier::Embed,
            when: When::Maybe,
            title: "Add semantic search for fewer agent turns".into(),
            why: format!(
                "~{}M tokens: lexical stays accurate, but embeddings cut agent turns (~2 vs ~4.5) and enable pre-retrieval for hosts (S4)",
                p.tokens / 1_000_000
            ),
            commands: enable(),
        });
    } else {
        skip(
            Tier::Embed,
            "one language and under 1M tokens: lexical tools are enough (S4)".into(),
        );
    }

    // 4. Fine-tuning.
    if fits {
        skip(Tier::Tune, "the bundle fits in context".into());
    } else if !cross {
        skip(
            Tier::Tune,
            "no cross-language questions expected, where fine-tuning helps most (S11)".into(),
        );
    } else if p.docs < TUNE_MIN_DOCS {
        skip(
            Tier::Tune,
            format!(
                "{} documents; fine-tuning needs about {TUNE_MIN_DOCS}+ (S11)",
                p.docs
            ),
        );
    } else {
        let mut why = "on its own documents a tuned model gained +6 R@1 (+10.5 cross-language) after Q4 export (S11); the workflow measures it against the base model before switching".to_owned();
        if o.private {
            why.push_str(". Private bundle: question generation sends passages to your agent's LLM, so use a local model or skip");
        }
        steps.push(Step {
            tier: Tier::Tune,
            when: When::Maybe,
            title: "Fine-tune the embedding model on this bundle".into(),
            why,
            commands: needs_build(vec![
                "ask your agent: \"tune embeddings for this bundle\" (skill okbase-tune)".into(),
                "okbase embed tune guide   # the steps, for agents without the skill".into(),
            ]),
        });
    }

    Advice {
        profile,
        steps,
        skipped,
    }
}

impl Advice {
    /// Human-readable report.
    pub fn to_text(&self) -> String {
        let p = &self.profile;
        let mut langs: Vec<_> = p.langs.iter().collect();
        langs.sort_by(|a, b| b.1.total_cmp(a.1));
        let langs = langs
            .iter()
            .map(|(k, v)| format!("{k} {:.0}%", *v * 100.0))
            .collect::<Vec<_>>()
            .join(" / ");
        let level = p.level.map_or("below L0".to_owned(), |l| l.to_string());
        let mut out = format!(
            "Bundle: {} docs, ~{} tokens, {langs}, level {level}{}{}\n\nRecommended path\n",
            p.docs,
            p.tokens,
            if p.missing_descriptions > 0 {
                format!(" ({} without description)", p.missing_descriptions)
            } else {
                String::new()
            },
            if p.datasets { ", has tables" } else { "" },
        );
        for (i, s) in self.steps.iter().enumerate() {
            let when = match s.when {
                When::Now => "now",
                When::Next => "next",
                When::Maybe => "maybe",
                When::Done => "done",
            };
            out.push_str(&format!(
                "  {}. [{when}] {}\n     why: {}\n",
                i + 1,
                s.title,
                s.why
            ));
            for c in &s.commands {
                out.push_str(&format!("     $ {c}\n"));
            }
        }
        if !self.skipped.is_empty() {
            out.push_str("\nNot needed\n");
            for s in &self.skipped {
                out.push_str(&format!("  - {:?}: {}\n", s.tier, s.why));
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn profile(docs: usize, tokens: usize, langs: &[(&str, f64)]) -> Profile {
        Profile {
            docs,
            tokens,
            chunks: docs * 2,
            langs: langs.iter().map(|(k, v)| ((*k).into(), *v)).collect(),
            level: Some(Level::L2),
            missing_descriptions: 0,
            has_index: true,
            datasets: false,
            embed: None,
            build: BuildFeatures {
                embed_local: true,
                embed_api: false,
                ja_embedded: false,
                import: true,
            },
        }
    }

    fn tiers(a: &Advice) -> Vec<(Tier, When)> {
        a.steps.iter().map(|s| (s.tier, s.when)).collect()
    }

    #[test]
    fn small_bundle_goes_full_context() {
        let a = plan(
            profile(20, 12_000, &[("en", 1.0)]),
            &AdviseOptions::default(),
        );
        assert_eq!(tiers(&a), [(Tier::Full, When::Now)]);
        assert!(a.skipped.iter().any(|s| s.tier == Tier::Embed));
    }

    #[test]
    fn single_language_mid_size_stays_lexical() {
        let a = plan(
            profile(300, 400_000, &[("en", 1.0)]),
            &AdviseOptions::default(),
        );
        assert_eq!(tiers(&a), [(Tier::Lexical, When::Now)]);
    }

    #[test]
    fn large_single_language_may_embed() {
        let a = plan(
            profile(3000, 4_400_000, &[("en", 1.0)]),
            &AdviseOptions::default(),
        );
        assert_eq!(
            tiers(&a),
            [(Tier::Lexical, When::Now), (Tier::Embed, When::Maybe)]
        );
    }

    #[test]
    fn cross_language_users_get_embed_then_tune() {
        let o = AdviseOptions {
            user_langs: vec!["vi".into(), "ja".into()],
            ..Default::default()
        };
        let a = plan(profile(287, 1_100_000, &[("en", 0.97), ("vi", 0.03)]), &o);
        assert_eq!(
            tiers(&a),
            [
                (Tier::Lexical, When::Now),
                (Tier::Embed, When::Next),
                (Tier::Tune, When::Maybe)
            ]
        );
        assert!(a.steps[1].why.contains("vi/ja"), "{}", a.steps[1].why);
    }

    #[test]
    fn mixed_bundle_below_tune_minimum() {
        let a = plan(
            profile(60, 90_000, &[("vi", 0.5), ("ja", 0.3), ("en", 0.2)]),
            &AdviseOptions::default(),
        );
        assert_eq!(
            tiers(&a),
            [(Tier::Lexical, When::Now), (Tier::Embed, When::Next)]
        );
        assert!(
            a.skipped
                .iter()
                .any(|s| s.tier == Tier::Tune && s.why.contains("60 documents"))
        );
    }

    #[test]
    fn curation_and_tables_come_early() {
        let mut p = profile(200, 300_000, &[("en", 1.0)]);
        p.level = Some(Level::L1);
        p.missing_descriptions = 50;
        p.datasets = true;
        let a = plan(p, &AdviseOptions::default());
        assert_eq!(
            tiers(&a),
            [
                (Tier::Lexical, When::Now),
                (Tier::Curate, When::Now),
                (Tier::Data, When::Now)
            ]
        );
        let mut p = profile(200, 300_000, &[("en", 1.0)]);
        p.level = None;
        assert!(plan(p, &AdviseOptions::default()).steps[1].commands[0].contains("adopt"));
    }

    #[test]
    fn embedded_bundle_is_done_and_missing_build_is_named() {
        let mut p = profile(287, 1_100_000, &[("en", 0.9), ("ja", 0.1)]);
        p.embed = Some(crate::EmbedStatus {
            model: "embeddinggemma-300m-q4".into(),
            chunks: 10,
            embedded: 10,
        });
        let a = plan(p, &AdviseOptions::default());
        assert_eq!(a.steps[1].when, When::Done);
        let mut p = profile(287, 1_100_000, &[("en", 0.9), ("ja", 0.1)]);
        p.build.embed_local = false;
        let a = plan(p, &AdviseOptions::default());
        assert!(a.steps[1].commands[0].contains("okbase-full"));
    }
}
