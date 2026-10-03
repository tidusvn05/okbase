//! Fine-tuning data for okbase embeddings (docs/PLAN-advise-tune.md §2–3).
//!
//! okbase never calls an LLM: the user's agent (Claude Code, Codex, …) writes the questions.
//! This crate samples passages from the index, splits them into small batches with a short
//! prompt, checks every submitted question against the standard ([`standard`]) and writes the
//! training pairs and the held-out evaluation set. Everything lives in a run directory under
//! the state dir (`<state>/tune/<run>/`); the bundle is never written.
//!
//! Files of a run: `plan.json`, `batches/NNNN.json` (passages), `claims/NNNN` (an agent is on
//! it), `answers/NNNN.jsonl` (accepted questions), then `train.jsonl` and `heldout.jsonl`.

#![forbid(unsafe_code)]

#[cfg(feature = "train")]
pub mod python;
pub mod standard;

use std::collections::{BTreeMap, HashSet};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

pub use standard::{Kind, Passage, Question, STANDARD, Settings, check_batch, normalize, prompt};

/// Errors.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum Error {
    /// Reading the index failed.
    #[error("index: {0}")]
    Index(#[from] rusqlite::Error),
    /// A read API failed.
    #[error(transparent)]
    Query(#[from] okbase_query::Error),
    /// A file of the run could not be read or written.
    #[error("{path}: {message}")]
    Io {
        /// Path.
        path: PathBuf,
        /// Message.
        message: String,
    },
    /// The request cannot be served (unknown batch, no run, nothing to sample…).
    #[error("{0}")]
    Invalid(String),
}

fn io(path: &Path, e: impl std::fmt::Display) -> Error {
    Error::Io {
        path: path.to_owned(),
        message: e.to_string(),
    }
}

fn read(path: &Path) -> Result<String, Error> {
    std::fs::read_to_string(path).map_err(|e| io(path, e))
}

fn write(path: &Path, text: &str) -> Result<(), Error> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| io(parent, e))?;
    }
    let tmp = path.with_extension("tmp");
    std::fs::write(&tmp, text).map_err(|e| io(&tmp, e))?;
    std::fs::rename(&tmp, path).map_err(|e| io(path, e))
}

fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
}

/// A claim older than this is free again (the agent probably stopped).
pub const CLAIM_SECONDS: u64 = 30 * 60;

/// `plan.json`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Plan {
    /// Question standard.
    pub standard: String,
    /// Run id.
    pub id: String,
    /// Creation time (Unix seconds).
    pub created: u64,
    /// Settings.
    pub settings: Settings,
    /// Passages sampled.
    pub passages: usize,
    /// Documents with sampled passages / held out.
    pub docs: usize,
    /// Held-out documents.
    pub heldout_docs: Vec<String>,
    /// Number of batches.
    pub batches: usize,
}

/// A batch as printed for the agent.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct BatchView {
    /// Batch number (1-based).
    pub batch: usize,
    /// Batches in the run.
    pub of: usize,
    /// Instructions.
    pub prompt: String,
    /// Passages.
    pub passages: Vec<PassageView>,
    /// How to submit.
    pub submit: String,
}

/// A passage as printed for the agent.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct PassageView {
    /// Key to use in answers.
    pub key: String,
    /// Questions to write.
    pub questions: usize,
    /// Passage language.
    pub lang: String,
    /// `title > heading`.
    pub title: String,
    /// Text.
    pub text: String,
}

impl BatchView {
    /// Plain-text form.
    pub fn to_text(&self) -> String {
        let mut out = format!(
            "okbase question standard v1, batch {} of {}\n\n{}\n",
            self.batch, self.of, self.prompt
        );
        for p in &self.passages {
            out.push_str(&format!(
                "\n--- passage {} [{}] lang: {} | {}\n{}\n",
                p.key, p.questions, p.lang, p.title, p.text
            ));
        }
        out.push_str(&format!("\nSubmit: {}\n", self.submit));
        out
    }
}

/// Result of a submission.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Submission {
    /// Batch number.
    pub batch: usize,
    /// Whether the batch was accepted (all or nothing).
    pub accepted: bool,
    /// Questions accepted.
    pub questions: usize,
    /// What to fix, when rejected.
    pub errors: Vec<String>,
    /// Batches still without answers.
    pub remaining: usize,
    /// What to do next.
    pub next: String,
}

/// Progress of a run.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Status {
    /// Run id.
    pub run: String,
    /// Run directory.
    pub dir: PathBuf,
    /// Question standard.
    pub standard: String,
    /// Languages.
    pub langs: Vec<String>,
    /// Passages.
    pub passages: usize,
    /// Batches in total / answered / claimed by an agent right now.
    pub batches: usize,
    /// Answered batches.
    pub answered: usize,
    /// Claimed, unanswered batches.
    pub claimed: usize,
    /// Accepted questions for training.
    pub train_pairs: usize,
    /// Accepted questions on held-out documents.
    pub heldout_questions: usize,
    /// Questions per kind.
    pub kinds: BTreeMap<String, usize>,
    /// Questions per language.
    pub langs_written: BTreeMap<String, usize>,
    /// Every batch answered and enough training pairs.
    pub ready: bool,
    /// Why not ready, or what is next.
    pub next: String,
}

/// A fine-tuning run.
#[derive(Debug, Clone)]
pub struct Run {
    dir: PathBuf,
    plan: Plan,
}

/// `<state>/tune`.
pub fn runs_dir(state: &Path) -> PathBuf {
    state.join("tune")
}

/// Runs, oldest first.
pub fn runs(state: &Path) -> Vec<String> {
    let mut v: Vec<String> = std::fs::read_dir(runs_dir(state))
        .into_iter()
        .flatten()
        .flatten()
        .filter(|e| e.path().join("plan.json").is_file())
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .collect();
    v.sort();
    v
}

struct Candidate {
    passage: Passage,
    stratum: (String, String),
    rank_in_doc: usize,
    hash: String,
}

fn hash(s: &str) -> String {
    blake3::hash(s.as_bytes()).to_hex().to_string()
}

/// Samples passages and creates a run (does not touch the bundle).
pub fn init(
    index: &okbase_index::Index,
    scope: &okbase_query::Scope,
    state: &Path,
    settings: Settings,
) -> Result<Run, Error> {
    if settings.langs.is_empty() {
        return Err(Error::Invalid("at least one language is needed".into()));
    }
    let visible = okbase_query::visible_ids(index, scope, None)?;
    let conn = index.connection();
    let mut st = conn.prepare(
        "SELECT c.doc_id, c.ord, c.heading, c.text, c.tokens, d.title, d.lang, \
         (SELECT COUNT(*) FROM chunks x WHERE x.doc_id = c.doc_id) \
         FROM chunks c JOIN docs d ON d.id = c.doc_id WHERE d.reserved = 0 ORDER BY c.doc_id, c.ord",
    )?;
    let rows = st.query_map([], |r| {
        Ok((
            r.get::<_, String>(0)?,
            r.get::<_, i64>(1)?,
            r.get::<_, String>(2)?,
            r.get::<_, String>(3)?,
            r.get::<_, i64>(4)?,
            r.get::<_, String>(5)?,
            r.get::<_, Option<String>>(6)?,
            r.get::<_, i64>(7)?,
        ))
    })?;
    let mut by_doc: BTreeMap<String, Vec<Candidate>> = BTreeMap::new();
    for row in rows {
        let (doc, ord, heading, text, tokens, title, lang, n) = row?;
        let tokens = usize::try_from(tokens).unwrap_or(0);
        if !visible.contains(&doc) || tokens < settings.min_chunk_tokens || text.trim().is_empty() {
            continue;
        }
        let lang = lang
            .filter(|l| !l.is_empty())
            .map(|l| {
                l.to_lowercase()
                    .split(['-', '_'])
                    .next()
                    .unwrap_or("other")
                    .to_owned()
            })
            .unwrap_or_else(|| {
                okbase_analyze::detect_lang(&text.chars().take(2000).collect::<String>())
                    .map_or("other", |l| l.code())
                    .to_owned()
            });
        let key = format!("{doc}#{ord}");
        let top = doc.split('/').next().unwrap_or("").to_owned();
        by_doc.entry(doc.clone()).or_default().push(Candidate {
            hash: hash(&key),
            stratum: (
                if doc.contains('/') {
                    top
                } else {
                    String::new()
                },
                lang.clone(),
            ),
            rank_in_doc: 0,
            passage: Passage {
                key,
                doc,
                title,
                heading,
                text,
                lang,
                tokens,
                whole: n == 1,
                heldout: false,
            },
        });
    }
    if by_doc.is_empty() {
        return Err(Error::Invalid(
            "no passages to sample (empty bundle, or every chunk is too short)".into(),
        ));
    }
    // Stratified, deterministic sample: strata (top folder × language) take turns; within a
    // stratum, first chunks of documents come before second chunks, in hash order.
    let mut strata: BTreeMap<(String, String), Vec<Candidate>> = BTreeMap::new();
    for (_, mut cands) in by_doc {
        cands.sort_by(|a, b| a.hash.cmp(&b.hash));
        for (i, mut c) in cands.into_iter().enumerate() {
            c.rank_in_doc = i;
            strata.entry(c.stratum.clone()).or_default().push(c);
        }
    }
    let mut queues: Vec<std::collections::VecDeque<Candidate>> = strata
        .into_values()
        .map(|mut v| {
            v.sort_by(|a, b| (a.rank_in_doc, &a.hash).cmp(&(b.rank_in_doc, &b.hash)));
            v.into()
        })
        .collect();
    let mut picked = Vec::new();
    while picked.len() < settings.max_passages && queues.iter().any(|q| !q.is_empty()) {
        for q in &mut queues {
            if picked.len() >= settings.max_passages {
                break;
            }
            if let Some(c) = q.pop_front() {
                picked.push(c.passage);
            }
        }
    }
    // Held-out documents.
    let mut docs: Vec<String> = picked.iter().map(|p| p.doc.clone()).collect();
    docs.sort();
    docs.dedup();
    let n = docs.len();
    let want = ((n as f64 * settings.heldout_share).ceil() as usize)
        .max(settings.min_heldout_docs.min(n / 4))
        .min(n.saturating_sub(1));
    let mut ranked = docs.clone();
    ranked.sort_by_key(|d| hash(&format!("heldout:{d}")));
    let heldout: HashSet<String> = ranked.into_iter().take(want).collect();
    for p in &mut picked {
        p.heldout = heldout.contains(&p.doc);
    }
    // Batches, in sample order.
    let mut batches: Vec<Vec<Passage>> = Vec::new();
    let mut cur: Vec<Passage> = Vec::new();
    let mut cur_tokens = 0;
    for p in picked.iter().cloned() {
        if !cur.is_empty()
            && (cur.len() >= settings.batch_passages
                || cur_tokens + p.tokens > settings.batch_tokens)
        {
            batches.push(std::mem::take(&mut cur));
            cur_tokens = 0;
        }
        cur_tokens += p.tokens;
        cur.push(p);
    }
    if !cur.is_empty() {
        batches.push(cur);
    }
    let created = now();
    let mut id = format!("run-{created}");
    let root = runs_dir(state);
    while root.join(&id).exists() {
        id.push('b');
    }
    let dir = root.join(&id);
    let mut heldout_docs: Vec<String> = heldout.into_iter().collect();
    heldout_docs.sort();
    let plan = Plan {
        standard: STANDARD.into(),
        id,
        created,
        settings,
        passages: picked.len(),
        docs: n,
        heldout_docs,
        batches: batches.len(),
    };
    for (i, b) in batches.iter().enumerate() {
        let path = dir.join("batches").join(format!("{:04}.json", i + 1));
        write(
            &path,
            &serde_json::to_string_pretty(b).map_err(|e| io(&path, e))?,
        )?;
    }
    let path = dir.join("plan.json");
    write(
        &path,
        &serde_json::to_string_pretty(&plan).map_err(|e| io(&path, e))?,
    )?;
    Ok(Run { dir, plan })
}

impl Run {
    /// Opens a run directory.
    pub fn open(dir: &Path) -> Result<Run, Error> {
        let path = dir.join("plan.json");
        let plan: Plan = serde_json::from_str(&read(&path)?).map_err(|e| io(&path, e))?;
        Ok(Run {
            dir: dir.to_owned(),
            plan,
        })
    }

    /// Opens a run by id, or the newest run.
    pub fn find(state: &Path, id: Option<&str>) -> Result<Run, Error> {
        let id = match id {
            Some(id) => id.to_owned(),
            None => runs(state).pop().ok_or_else(|| {
                Error::Invalid(
                    "no fine-tuning run yet; start one with `okbase embed tune init`".into(),
                )
            })?,
        };
        let dir = runs_dir(state).join(&id);
        if !dir.join("plan.json").is_file() {
            return Err(Error::Invalid(format!(
                "no run `{id}`; see `okbase embed tune runs`"
            )));
        }
        Run::open(&dir)
    }

    /// Directory.
    pub fn dir(&self) -> &Path {
        &self.dir
    }

    /// Plan.
    pub fn plan(&self) -> &Plan {
        &self.plan
    }

    fn batch_path(&self, n: usize) -> PathBuf {
        self.dir.join("batches").join(format!("{n:04}.json"))
    }

    fn answer_path(&self, n: usize) -> PathBuf {
        self.dir.join("answers").join(format!("{n:04}.jsonl"))
    }

    fn claim_path(&self, n: usize) -> PathBuf {
        self.dir.join("claims").join(format!("{n:04}"))
    }

    /// The passages of a batch.
    pub fn batch(&self, n: usize) -> Result<Vec<Passage>, Error> {
        if n == 0 || n > self.plan.batches {
            return Err(Error::Invalid(format!(
                "no batch {n}; this run has batches 1–{}",
                self.plan.batches
            )));
        }
        let path = self.batch_path(n);
        serde_json::from_str(&read(&path)?).map_err(|e| io(&path, e))
    }

    fn answered(&self, n: usize) -> bool {
        self.answer_path(n).is_file()
    }

    fn claimed(&self, n: usize) -> bool {
        std::fs::read_to_string(self.claim_path(n))
            .ok()
            .and_then(|t| t.trim().parse::<u64>().ok())
            .is_some_and(|t| now().saturating_sub(t) < CLAIM_SECONDS)
    }

    /// The batch as printed for an agent.
    pub fn view(&self, n: usize) -> Result<BatchView, Error> {
        let passages = self.batch(n)?;
        let s = &self.plan.settings;
        Ok(BatchView {
            batch: n,
            of: self.plan.batches,
            prompt: prompt(s),
            passages: passages
                .iter()
                .map(|p| PassageView {
                    key: p.key.clone(),
                    questions: p.quota(s),
                    lang: p.lang.clone(),
                    title: okbase_search::chunk_input(&p.title, &p.heading, "").0,
                    text: p.text.clone(),
                })
                .collect(),
            submit: format!(
                "okbase embed tune submit {n} <file.jsonl>   (or pipe the JSONL into: okbase embed tune submit {n} -)"
            ),
        })
    }

    /// The next batch without answers that no agent is working on; with `claim`, marks it as
    /// taken for [`CLAIM_SECONDS`] so parallel agents get different batches.
    pub fn next(&self, claim: bool) -> Result<Option<BatchView>, Error> {
        for n in 1..=self.plan.batches {
            if self.answered(n) || self.claimed(n) {
                continue;
            }
            if claim {
                write(&self.claim_path(n), &now().to_string())?;
            }
            return self.view(n).map(Some);
        }
        Ok(None)
    }

    fn seen_except(&self, skip: usize) -> Result<HashSet<String>, Error> {
        let mut seen = HashSet::new();
        for n in 1..=self.plan.batches {
            if n == skip || !self.answered(n) {
                continue;
            }
            for q in self.answers(n)? {
                seen.insert(normalize(&q.q));
            }
        }
        Ok(seen)
    }

    fn answers(&self, n: usize) -> Result<Vec<Question>, Error> {
        let path = self.answer_path(n);
        read(&path)?
            .lines()
            .filter(|l| !l.trim().is_empty())
            .map(|l| serde_json::from_str(l).map_err(|e| io(&path, e)))
            .collect()
    }

    /// Checks and stores the questions for batch `n` (all or nothing). A batch can be submitted
    /// again to replace its answers. `extra_seen` holds normalized questions that must not be
    /// repeated (e.g. a human-written eval set).
    pub fn submit(
        &self,
        n: usize,
        jsonl: &str,
        extra_seen: &HashSet<String>,
    ) -> Result<Submission, Error> {
        let passages = self.batch(n)?;
        let mut seen = self.seen_except(n)?;
        seen.extend(extra_seen.iter().cloned());
        let (questions, errors) = check_batch(&passages, &self.plan.settings, jsonl, &seen);
        let accepted = errors.is_empty();
        if accepted {
            let mut text = String::new();
            for q in &questions {
                text.push_str(&serde_json::to_string(q).map_err(|e| io(&self.answer_path(n), e))?);
                text.push('\n');
            }
            write(&self.answer_path(n), &text)?;
            let _ = std::fs::remove_file(self.claim_path(n));
        }
        let remaining = (1..=self.plan.batches)
            .filter(|b| !self.answered(*b))
            .count();
        let next = if !accepted {
            format!("fix the lines above and submit batch {n} again (the whole batch)")
        } else if remaining > 0 {
            "okbase embed tune next".into()
        } else {
            "okbase embed tune check".into()
        };
        Ok(Submission {
            batch: n,
            accepted,
            questions: if accepted { questions.len() } else { 0 },
            errors,
            remaining,
            next,
        })
    }

    /// Progress.
    pub fn status(&self) -> Result<Status, Error> {
        let (mut answered, mut claimed, mut train, mut heldout) = (0, 0, 0, 0);
        let mut kinds = BTreeMap::new();
        let mut langs = BTreeMap::new();
        for n in 1..=self.plan.batches {
            if !self.answered(n) {
                claimed += usize::from(self.claimed(n));
                continue;
            }
            answered += 1;
            let passages = self.batch(n)?;
            for q in self.answers(n)? {
                let held = passages.iter().any(|p| p.key == q.passage && p.heldout);
                if held {
                    heldout += 1;
                } else {
                    train += 1;
                }
                *kinds.entry(q.kind.name().to_owned()).or_default() += 1;
                *langs.entry(q.lang.clone()).or_default() += 1;
            }
        }
        let s = &self.plan.settings;
        let all = answered == self.plan.batches;
        let ready = all && train >= s.min_pairs;
        let next = if !all {
            format!(
                "{} of {} batches left: okbase embed tune next",
                self.plan.batches - answered,
                self.plan.batches
            )
        } else if !ready {
            format!(
                "only {train} training pairs (standard: at least {}); the bundle may be too small to fine-tune",
                s.min_pairs
            )
        } else if self.train_path().is_file() {
            "okbase embed tune train".into()
        } else {
            "okbase embed tune check, then okbase embed tune train".into()
        };
        Ok(Status {
            run: self.plan.id.clone(),
            dir: self.dir.clone(),
            standard: self.plan.standard.clone(),
            langs: s.langs.clone(),
            passages: self.plan.passages,
            batches: self.plan.batches,
            answered,
            claimed,
            train_pairs: train,
            heldout_questions: heldout,
            kinds,
            langs_written: langs,
            ready,
            next,
        })
    }

    /// When every batch is answered, writes `train.jsonl` (query, title, text: the exact inputs
    /// search embeds) and `heldout.jsonl` (q, doc, lang, doc_lang, kind). Returns the status.
    pub fn finalize(&self) -> Result<Status, Error> {
        let status = self.status()?;
        if status.answered < status.batches {
            return Ok(status);
        }
        let (mut train, mut held) = (String::new(), String::new());
        for n in 1..=self.plan.batches {
            let passages = self.batch(n)?;
            for q in self.answers(n)? {
                let Some(p) = passages.iter().find(|p| p.key == q.passage) else {
                    continue;
                };
                let line = if p.heldout {
                    serde_json::json!({"q": q.q, "doc": p.doc, "lang": q.lang, "doc_lang": p.lang, "kind": q.kind})
                } else {
                    let (title, text) = okbase_search::chunk_input(&p.title, &p.heading, &p.text);
                    serde_json::json!({"query": q.q, "title": title, "text": text, "lang": q.lang, "kind": q.kind, "passage": p.key})
                };
                let out = if p.heldout { &mut held } else { &mut train };
                out.push_str(&line.to_string());
                out.push('\n');
            }
        }
        write(&self.dir.join("train.jsonl"), &train)?;
        write(&self.dir.join("heldout.jsonl"), &held)?;
        Ok(status)
    }

    /// `train.jsonl`, once finalized.
    pub fn train_path(&self) -> PathBuf {
        self.dir.join("train.jsonl")
    }

    /// `heldout.jsonl`, once finalized.
    pub fn heldout_path(&self) -> PathBuf {
        self.dir.join("heldout.jsonl")
    }
}

/// A Colab notebook that trains the adapter on a GPU from `train_jsonl` and downloads it
/// (the `colab` backend of `okbase embed tune train`).
pub fn colab_notebook(train_jsonl: &str, script: &str, requirements: &str) -> String {
    let cell = |kind: &str, src: String| {
        serde_json::json!({
            "cell_type": kind, "metadata": {}, "source": src,
            "outputs": if kind == "code" { serde_json::json!([]) } else { serde_json::Value::Null },
            "execution_count": serde_json::Value::Null,
        })
    };
    let pins: Vec<&str> = requirements
        .lines()
        .filter(|l| !l.starts_with('#') && !l.trim().is_empty())
        .filter(|l| !l.starts_with("numpy")) // keep Colab's numpy, built for its torch
        .collect();
    let mut cells = vec![
        cell("markdown", "# okbase embed tune (GPU)\n\nRuntime → Change runtime type → T4 GPU, then Run all. \
              The last cell downloads two files; put them in one folder and run \
              `okbase embed tune import <folder>`, then `okbase embed tune export`.".into()),
        cell("code", format!("!pip install -q unsloth==2026.9.14 {}", pins.join(" "))),
        cell("code", format!("%%writefile okbase_tune.py\n{script}")),
        cell("code", format!("%%writefile train.jsonl\n{train_jsonl}")),
        cell("code", "!python okbase_tune.py train train.jsonl out".into()),
        cell("code", "from google.colab import files\nfor f in ('adapter_config.json', 'adapter_model.safetensors'):\n    files.download(f'out/adapter/{f}')".into()),
    ];
    for c in &mut cells {
        if c["cell_type"] == "markdown" {
            c.as_object_mut().map(|o| {
                o.remove("outputs");
                o.remove("execution_count")
            });
        }
    }
    serde_json::to_string_pretty(&serde_json::json!({
        "nbformat": 4, "nbformat_minor": 5,
        "metadata": {"accelerator": "GPU", "kernelspec": {"name": "python3", "display_name": "Python 3"}},
        "cells": cells,
    }))
    .unwrap_or_default()
}

/// The full guide for agents without the okbase-tune skill (`okbase embed tune guide`).
pub const GUIDE: &str = include_str!("guide.md");

#[cfg(test)]
mod tests;
