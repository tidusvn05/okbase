//! Fine-tuning support in the facade: runs live in the bundle's state dir, and
//! [`eval_models`] measures retrieval of several models on the same questions
//! (docs/plans/advise-tune.md §2).

use std::collections::{BTreeMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use serde::{Deserialize, Serialize};

use crate::{Bundle, Error, OpenOptions, Scope, SearchRequest, StateDir};

#[cfg(feature = "embed-tune")]
pub use okbase_tune::python::{DOWNLOAD_HINT as PYTHON_DOWNLOAD_HINT, PyEnv};
pub use okbase_tune::{
    BatchView, GUIDE, Kind, Plan, Run, STANDARD, Settings, Status as TuneStatus, Submission,
};

/// Human-written evaluation questions, preferred over the generated held-out set.
pub const HUMAN_EVAL_PATH: &str = "_meta/eval/questions.jsonl";

impl From<okbase_tune::Error> for Error {
    fn from(e: okbase_tune::Error) -> Self {
        Error::Tune(e.to_string())
    }
}

impl Bundle {
    /// Where runs and other state live (the index directory).
    pub fn state_path(&self) -> Result<PathBuf, Error> {
        self.index_path()
            .and_then(|p| p.parent().map(Path::to_owned))
            .ok_or_else(|| Error::Tune("an in-memory bundle has no state directory".into()))
    }

    /// Languages for a run when none are given: those making up ≥ 5% of the bundle, plus `en`.
    pub fn default_tune_langs(&self, scope: &Scope) -> Result<Vec<String>, Error> {
        let langs = okbase_query::content_langs(&self.index(), scope)?;
        let total = langs.values().sum::<usize>().max(1) as f64;
        let mut out: Vec<(String, usize)> = langs
            .into_iter()
            .filter(|(k, v)| k != "other" && *v as f64 / total >= crate::advise::LANG_MIN_SHARE)
            .collect();
        out.sort_by_key(|a| std::cmp::Reverse(a.1));
        let mut out: Vec<String> = out.into_iter().map(|(k, _)| k).collect();
        if !out.iter().any(|l| l == "en") {
            out.push("en".into());
        }
        Ok(out)
    }

    /// Samples passages and starts a fine-tuning run (writes only to the state dir).
    pub fn tune_init(&self, settings: Settings, scope: &Scope) -> Result<Run, Error> {
        let state = self.state_path()?;
        Ok(okbase_tune::init(&self.index(), scope, &state, settings)?)
    }

    /// Opens a run (`None`: the newest).
    pub fn tune_run(&self, id: Option<&str>) -> Result<Run, Error> {
        Ok(Run::find(&self.state_path()?, id)?)
    }

    /// Run ids, oldest first.
    pub fn tune_runs(&self) -> Result<Vec<String>, Error> {
        Ok(okbase_tune::runs(&self.state_path()?))
    }

    /// Normalized human-written eval questions (must not be reused for training).
    pub fn human_eval_questions(&self) -> Result<HashSet<String>, Error> {
        let path = self.root().join(HUMAN_EVAL_PATH);
        if !path.is_file() {
            return Ok(HashSet::new());
        }
        Ok(load_questions(&path)?
            .iter()
            .map(|q| okbase_tune::normalize(&q.q))
            .collect())
    }
}

/// An evaluation question: the document that answers it, and the languages involved.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EvalQuestion {
    /// The question.
    pub q: String,
    /// Id of the document that answers it.
    pub doc: String,
    /// Language of the question.
    #[serde(default)]
    pub lang: Option<String>,
    /// Language of the document.
    #[serde(default)]
    pub doc_lang: Option<String>,
}

impl EvalQuestion {
    fn cross(&self) -> Option<bool> {
        Some(self.lang.as_ref()? != self.doc_lang.as_ref()?)
    }
}

/// Reads JSONL eval questions (`{"q", "doc", "lang"?, "doc_lang"?}` per line).
pub fn load_questions(path: &Path) -> Result<Vec<EvalQuestion>, Error> {
    let text = std::fs::read_to_string(path)
        .map_err(|e| Error::Tune(format!("{}: {e}", path.display())))?;
    text.lines()
        .enumerate()
        .filter(|(_, l)| !l.trim().is_empty())
        .map(|(i, l)| {
            serde_json::from_str(l).map_err(|e| {
                Error::Tune(format!(
                    "{}:{}: {e} (expected {{\"q\", \"doc\", \"lang\"?, \"doc_lang\"?}})",
                    path.display(),
                    i + 1
                ))
            })
        })
        .collect()
}

/// Retrieval quality of one model.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ModelScore {
    /// Model id as configured (`embeddinggemma-300m-q4`, `custom:…`).
    pub model: String,
    /// Questions.
    pub n: usize,
    /// Document ranked first.
    pub r_at_1: f64,
    /// Document in the top 3.
    pub r_at_3: f64,
    /// Mean reciprocal rank (top 10).
    pub mrr: f64,
    /// R@1 on questions in another language than their document.
    pub cross_r_at_1: Option<f64>,
    /// Questions in another language than their document.
    pub cross_n: usize,
    /// R@1 on same-language questions.
    pub same_r_at_1: Option<f64>,
    /// Seconds spent embedding the bundle (0 when cached).
    pub embed_seconds: f64,
}

/// Several models on the same questions.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct EvalReport {
    /// Where the questions came from.
    pub questions: String,
    /// Scores, in the order asked.
    pub models: Vec<ModelScore>,
}

impl EvalReport {
    /// A table.
    pub fn to_text(&self) -> String {
        let pct = |v: Option<f64>| v.map_or("-".to_owned(), |v| format!("{v:.3}"));
        let mut out = format!(
            "questions: {}\n{:<36} {:>6} {:>6} {:>6} {:>10} {:>6}\n",
            self.questions, "model", "R@1", "R@3", "MRR", "cross R@1", "n"
        );
        for m in &self.models {
            out.push_str(&format!(
                "{:<36} {:>6.3} {:>6.3} {:>6.3} {:>10} {:>6}\n",
                m.model,
                m.r_at_1,
                m.r_at_3,
                m.mrr,
                pct(m.cross_r_at_1),
                m.n
            ));
        }
        out
    }
}

/// What [`eval_models`] measures.
#[derive(Debug, Clone)]
pub struct EvalRequest<'a> {
    /// Bundle root.
    pub root: &'a Path,
    /// Index location (as for [`Bundle::open`]).
    pub state_dir: StateDir,
    /// Vector cache file (default: the shared user cache).
    pub vector_cache: Option<PathBuf>,
    /// Models to compare (built-in ids or `custom:<name>`).
    pub models: &'a [String],
    /// Questions.
    pub questions: &'a [EvalQuestion],
    /// Where the questions came from (for the report).
    pub source: &'a str,
    /// Visible documents.
    pub scope: &'a Scope,
}

/// Embeds the bundle with each model (vectors are cached) and ranks documents for every
/// question, as `kb_search` does (one hit per document). Read-only for the bundle.
/// `progress(model, done, total)` reports embedding.
pub fn eval_models(
    req: &EvalRequest<'_>,
    progress: &mut dyn FnMut(&str, usize, usize),
) -> Result<EvalReport, Error> {
    let EvalRequest {
        root,
        state_dir,
        vector_cache,
        models,
        questions,
        source,
        scope,
    } = req.clone();
    if questions.is_empty() {
        return Err(Error::Tune(format!("no questions in {source}")));
    }
    let mut out = Vec::new();
    for model in models {
        let embedder = crate::embed::load_local_model(model)?;
        let mut opts = OpenOptions::default()
            .state_dir(state_dir.clone())
            .embedder(Arc::clone(&embedder));
        if let Some(c) = &vector_cache {
            opts = opts.vector_cache(c.clone());
        }
        let b = Bundle::open(root, opts)?;
        b.sync()?;
        let t = std::time::Instant::now();
        b.embed_sync(&mut |d, n| progress(model, d, n))?;
        let embed_seconds = t.elapsed().as_secs_f64();
        let mut agg: BTreeMap<&str, (usize, usize, usize, f64)> = BTreeMap::new();
        for q in questions {
            let r = b.search(
                &SearchRequest {
                    query: q.q.clone(),
                    limit: 10,
                    per_doc: 1,
                    filter: None,
                },
                scope,
            )?;
            let rank = r.hits.iter().position(|h| h.id == q.doc);
            let mut buckets = vec!["all"];
            match q.cross() {
                Some(true) => buckets.push("cross"),
                Some(false) => buckets.push("same"),
                None => {}
            }
            for bucket in buckets {
                let e = agg.entry(bucket).or_default();
                e.0 += 1;
                e.1 += usize::from(rank == Some(0));
                e.2 += usize::from(rank.is_some_and(|r| r < 3));
                e.3 += rank.map_or(0.0, |r| 1.0 / (r as f64 + 1.0));
            }
        }
        let r1 = |b: &str| agg.get(b).map(|e| e.1 as f64 / e.0 as f64);
        let all = agg["all"];
        out.push(ModelScore {
            model: model.clone(),
            n: all.0,
            r_at_1: all.1 as f64 / all.0 as f64,
            r_at_3: all.2 as f64 / all.0 as f64,
            mrr: all.3 / all.0 as f64,
            cross_r_at_1: r1("cross"),
            cross_n: agg.get("cross").map_or(0, |e| e.0),
            same_r_at_1: r1("same"),
            embed_seconds,
        });
    }
    Ok(EvalReport {
        questions: source.to_owned(),
        models: out,
    })
}

/// Where a run keeps the trained LoRA adapter.
pub fn adapter_dir(run: &Run) -> PathBuf {
    run.dir().join("model").join("adapter")
}

/// Files of a LoRA adapter.
pub const ADAPTER_FILES: [&str; 2] = ["adapter_config.json", "adapter_model.safetensors"];

/// Copies an adapter trained elsewhere (e.g. the Colab notebook) into the run.
pub fn import_adapter(run: &Run, from: &Path) -> Result<PathBuf, Error> {
    let to = adapter_dir(run);
    std::fs::create_dir_all(&to).map_err(|e| Error::Tune(format!("{}: {e}", to.display())))?;
    for f in ADAPTER_FILES {
        let src = from.join(f);
        if !src.is_file() {
            return Err(Error::Tune(format!(
                "{} is missing; download both {} from the notebook",
                src.display(),
                ADAPTER_FILES.join(" and ")
            )));
        }
        std::fs::copy(&src, to.join(f)).map_err(|e| Error::Tune(format!("{f}: {e}")))?;
    }
    Ok(to)
}

/// Checks that a run can be trained: finalized, enough pairs (unless `allow_small`), and the
/// Gemma license accepted (the base model is EmbeddingGemma).
pub fn check_trainable(run: &Run, allow_small: bool) -> Result<TuneStatus, Error> {
    let st = run.status()?;
    if st.answered < st.batches || !run.train_path().is_file() {
        return Err(Error::Tune(format!(
            "run {} is not finished: {}",
            st.run, st.next
        )));
    }
    if !st.ready && !allow_small {
        return Err(Error::Tune(format!(
            "{}; pass --allow-small to train anyway",
            st.next
        )));
    }
    let base = okbase_embed::find_model("embeddinggemma-300m-q4")
        .ok_or_else(|| Error::Tune("base model missing from this build".into()))?;
    if !okbase_embed::license_accepted(base) {
        return Err(Error::Tune(format!(
            "the base model is released under the {} ({}); accept it first with \
             `okbase embed models pull embeddinggemma-300m-q4 --accept-license`",
            base.license, base.license_url
        )));
    }
    Ok(st)
}

/// Writes a Colab notebook that trains the run on a GPU; returns its path.
#[cfg(feature = "embed-tune")]
pub fn colab_notebook(run: &Run) -> Result<PathBuf, Error> {
    use okbase_tune::python::{REQUIREMENTS, SCRIPT};
    let train = std::fs::read_to_string(run.train_path())
        .map_err(|e| Error::Tune(format!("{}: {e}", run.train_path().display())))?;
    let path = run.dir().join("colab").join("okbase-tune.ipynb");
    std::fs::create_dir_all(path.parent().unwrap_or(run.dir()))
        .map_err(|e| Error::Tune(e.to_string()))?;
    std::fs::write(
        &path,
        okbase_tune::colab_notebook(&train, SCRIPT, REQUIREMENTS),
    )
    .map_err(|e| Error::Tune(format!("{}: {e}", path.display())))?;
    Ok(path)
}

/// The Python environment for training: `Ok(None)` when it does not exist and `create` is false.
#[cfg(feature = "embed-tune")]
pub fn python_env(
    create: bool,
    log: &mut dyn FnMut(&str),
) -> Result<Option<okbase_tune::python::PyEnv>, Error> {
    let root = okbase_analyze::dict::user_cache_dir()
        .ok_or_else(|| Error::Tune("no user cache directory (set HOME)".into()))?;
    let gpu = okbase_tune::python::has_cuda();
    if let Some(e) = okbase_tune::python::existing(&root, gpu) {
        return Ok(Some(e));
    }
    if !create {
        return Ok(None);
    }
    Ok(Some(okbase_tune::python::create(&root, gpu, log)?))
}

/// Trains the run's adapter locally.
#[cfg(feature = "embed-tune")]
pub fn train(
    run: &Run,
    env: &okbase_tune::python::PyEnv,
    epochs: usize,
    log: &mut dyn FnMut(&str),
) -> Result<PathBuf, Error> {
    let out = run.dir().join("model");
    let (data, out_s) = (
        run.train_path().to_string_lossy().into_owned(),
        out.to_string_lossy().into_owned(),
    );
    okbase_tune::python::run_script(
        env,
        &["train", &data, &out_s, "--epochs", &epochs.to_string()],
        log,
    )?;
    Ok(adapter_dir(run))
}

/// The downloaded EmbeddingGemma Q4 files (reference graph and tokenizer), downloading them
/// first when this build can.
#[cfg(feature = "embed-tune")]
fn reference_snapshot() -> Result<PathBuf, Error> {
    let find = || -> Option<PathBuf> {
        let snaps = okbase_embed::models_dir()
            .ok()?
            .join("fastembed")
            .join("models--onnx-community--embeddinggemma-300m-ONNX")
            .join("snapshots");
        std::fs::read_dir(snaps)
            .ok()?
            .flatten()
            .map(|e| e.path())
            .find(|p| p.join("onnx").join("model_q4.onnx").exists())
    };
    if let Some(p) = find() {
        return Ok(p);
    }
    crate::embed::load_local_model("embeddinggemma-300m-q4")?;
    find().ok_or_else(|| Error::Tune("EmbeddingGemma Q4 files not found after download".into()))
}

/// Merges the adapter into EmbeddingGemma Q4 (ONNX), installs it as `custom:<name>` and
/// returns it. `name` defaults to `<bundle>-<run>`.
#[cfg(feature = "embed-tune")]
pub fn export(
    run: &Run,
    env: &okbase_tune::python::PyEnv,
    bundle_root: &Path,
    name: Option<&str>,
    log: &mut dyn FnMut(&str),
) -> Result<okbase_embed::CustomModel, Error> {
    let adapter = adapter_dir(run);
    if !adapter.join(ADAPTER_FILES[1]).is_file() {
        return Err(Error::Tune(format!(
            "no adapter in {}: run `okbase embed tune train` (or import one)",
            adapter.display()
        )));
    }
    // onnx refuses symlinked or hard-linked external data: copy the files.
    let snap = reference_snapshot()?;
    let reference = run.dir().join("reference");
    std::fs::create_dir_all(reference.join("onnx")).map_err(|e| Error::Tune(e.to_string()))?;
    for f in [
        "onnx/model_q4.onnx",
        "onnx/model_q4.onnx_data",
        "tokenizer.json",
        "config.json",
        "special_tokens_map.json",
        "tokenizer_config.json",
    ] {
        let to = reference.join(f);
        let _ = std::fs::remove_file(&to);
        std::fs::copy(snap.join(f), &to).map_err(|e| Error::Tune(format!("{f}: {e}")))?;
    }
    let name = name.map_or_else(|| default_model_name(bundle_root, run), str::to_owned);
    let train_hash = std::fs::read(run.train_path())
        .map(|b| blake3::hash(&b).to_hex().to_string())
        .unwrap_or_default();
    let st = run.status()?;
    let provenance = serde_json::json!({
        "run": st.run, "standard": st.standard, "langs": st.langs,
        "train_pairs": st.train_pairs, "train_hash": train_hash,
        "bundle": bundle_root.file_name().map(|n| n.to_string_lossy().into_owned()),
    });
    let out = run.dir().join("export");
    let _ = std::fs::remove_dir_all(&out);
    let s = |p: &Path| p.to_string_lossy().into_owned();
    okbase_tune::python::run_script(
        env,
        &[
            "export",
            &s(&adapter),
            &s(&reference),
            &s(&out),
            "--name",
            &name,
            "--provenance",
            &provenance.to_string(),
        ],
        log,
    )?;
    let _ = std::fs::remove_dir_all(&reference);
    let model = okbase_embed::install_custom(&out, None, true)?;
    record_exported_model(run, &format!("custom:{}", model.manifest.name))?;
    Ok(model)
}

/// `<bundle dir name>-<run number>`, lowercased to a valid model name.
pub fn default_model_name(bundle_root: &Path, run: &Run) -> String {
    let base = std::fs::canonicalize(bundle_root)
        .ok()
        .and_then(|p| p.file_name().map(|n| n.to_string_lossy().into_owned()))
        .unwrap_or_else(|| "bundle".into());
    let mut name: String = base
        .to_lowercase()
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || "-_.".contains(c) {
                c
            } else {
                '-'
            }
        })
        .collect();
    name = name
        .trim_matches(|c| c == '-' || c == '.')
        .chars()
        .take(40)
        .collect();
    if name.is_empty() {
        name = "bundle".into();
    }
    let id = &run.plan().id;
    format!("{name}-{}", id.trim_start_matches("run-"))
}

/// The tuned model must beat the base model by this much R@1 on the run's questions.
pub const GATE_MIN_GAIN: f64 = 0.02;
/// …and lose at most this much R@1 on the built-in general set.
pub const GATE_MAX_REGRESSION: f64 = 0.01;
/// The base model every run is trained from.
pub const BASE_MODEL: &str = "embeddinggemma-300m-q4";

/// The built-in general set: 100 short vi/en/ja documents and 300 questions
/// (`fixtures/multilingual`, synthetic). Catches a tuned model that got worse in general.
const REGRESSION: &str = include_str!("../data/regression.json");

#[derive(Deserialize)]
struct RegressionSet {
    source: String,
    docs: Vec<RegressionDoc>,
    questions: Vec<EvalQuestion>,
}

#[derive(Deserialize)]
struct RegressionDoc {
    path: String,
    content: String,
}

/// The model a run exported (`custom:<name>`), recorded by `export`.
pub fn exported_model(run: &Run) -> Option<String> {
    let text = std::fs::read_to_string(run.dir().join("model.json")).ok()?;
    serde_json::from_str::<serde_json::Value>(&text)
        .ok()?
        .get("model")?
        .as_str()
        .map(str::to_owned)
}

/// Records the exported model of a run.
pub fn record_exported_model(run: &Run, model: &str) -> Result<(), Error> {
    let path = run.dir().join("model.json");
    std::fs::write(
        &path,
        serde_json::json!({ "model": model }).to_string() + "\n",
    )
    .map_err(|e| Error::Tune(format!("{}: {e}", path.display())))
}

/// The outcome of `okbase embed tune eval`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Gate {
    /// Run id.
    pub run: String,
    /// Base model.
    pub base: String,
    /// Tuned model.
    pub tuned: String,
    /// Where the questions came from.
    pub questions: String,
    /// R@1 base → tuned on the questions.
    pub r_at_1: (f64, f64),
    /// Cross-language R@1 base → tuned (when the questions have languages).
    pub cross_r_at_1: Option<(f64, f64)>,
    /// Same-language R@1 base → tuned.
    pub same_r_at_1: Option<(f64, f64)>,
    /// R@1 base → tuned on the built-in general set.
    pub general_r_at_1: (f64, f64),
    /// Questions measured.
    pub n: usize,
    /// Whether activation is allowed.
    pub passed: bool,
    /// Why (one line per criterion).
    pub reasons: Vec<String>,
}

impl Gate {
    /// A short report.
    pub fn to_text(&self) -> String {
        let pair = |p: Option<(f64, f64)>| {
            p.map_or("-".to_owned(), |(a, b)| {
                format!("{a:.3} → {b:.3} ({:+.1})", (b - a) * 100.0)
            })
        };
        format!(
            "run {}: {} vs {}\nquestions:      {} ({})\nR@1:            {}\ncross-language: {}\nsame-language:  {}\ngeneral set:    {}\ngate:           {}\n{}\n",
            self.run,
            self.base,
            self.tuned,
            self.questions,
            self.n,
            pair(Some(self.r_at_1)),
            pair(self.cross_r_at_1),
            pair(self.same_r_at_1),
            pair(Some(self.general_r_at_1)),
            if self.passed { "PASSED" } else { "FAILED" },
            self.reasons
                .iter()
                .map(|r| format!("  - {r}"))
                .collect::<Vec<_>>()
                .join("\n")
        )
    }
}

/// Decides the gate from the two reports (base first, tuned second in each).
pub fn decide(run: &str, questions: &EvalReport, general: &EvalReport) -> Gate {
    let (b, t) = (&questions.models[0], &questions.models[1]);
    let (gb, gt) = (&general.models[0], &general.models[1]);
    let both = |x: Option<f64>, y: Option<f64>| x.zip(y);
    let mut reasons = Vec::new();
    let gain = t.r_at_1 - b.r_at_1;
    let ok_gain = gain >= GATE_MIN_GAIN - 1e-9;
    reasons.push(format!(
        "R@1 {:+.1} points on {} ({}: at least +{:.0})",
        gain * 100.0,
        questions.questions,
        if ok_gain { "ok" } else { "too small" },
        GATE_MIN_GAIN * 100.0
    ));
    let cross = both(b.cross_r_at_1, t.cross_r_at_1);
    let ok_cross = cross.is_none_or(|(x, y)| y >= x - 1e-9);
    if let Some((x, y)) = cross {
        reasons.push(format!(
            "cross-language R@1 {:+.1} ({})",
            (y - x) * 100.0,
            if ok_cross { "ok" } else { "dropped" }
        ));
    }
    let reg = gt.r_at_1 - gb.r_at_1;
    let ok_reg = reg >= -GATE_MAX_REGRESSION - 1e-9;
    reasons.push(format!(
        "general set R@1 {:+.1} ({}: at most -{:.0})",
        reg * 100.0,
        if ok_reg { "ok" } else { "regressed" },
        GATE_MAX_REGRESSION * 100.0
    ));
    Gate {
        run: run.to_owned(),
        base: b.model.clone(),
        tuned: t.model.clone(),
        questions: questions.questions.clone(),
        r_at_1: (b.r_at_1, t.r_at_1),
        cross_r_at_1: cross,
        same_r_at_1: both(b.same_r_at_1, t.same_r_at_1),
        general_r_at_1: (gb.r_at_1, gt.r_at_1),
        n: b.n,
        passed: ok_gain && ok_cross && ok_reg,
        reasons,
    }
}

/// Measures the run's tuned model against the base model on the human-written questions
/// (`_meta/eval/questions.jsonl`) or the run's held-out set, and on the built-in general set;
/// writes `<run>/eval.json`.
pub fn evaluate_run(
    bundle_root: &Path,
    state_dir: StateDir,
    run: &Run,
    scope: &Scope,
    progress: &mut dyn FnMut(&str, usize, usize),
) -> Result<Gate, Error> {
    let tuned = exported_model(run).ok_or_else(|| {
        Error::Tune(format!(
            "run {} has no exported model: run `okbase embed tune export`",
            run.plan().id
        ))
    })?;
    let human = bundle_root.join(HUMAN_EVAL_PATH);
    let (path, source) = if human.is_file() {
        (human, HUMAN_EVAL_PATH.to_owned())
    } else {
        (run.heldout_path(), "held-out questions".to_owned())
    };
    let questions = load_questions(&path)?;
    let models = vec![BASE_MODEL.to_owned(), tuned];
    let report = eval_models(
        &EvalRequest {
            root: bundle_root,
            state_dir,
            vector_cache: None,
            models: &models,
            questions: &questions,
            source: &source,
            scope,
        },
        progress,
    )?;
    let set: RegressionSet = serde_json::from_str(REGRESSION)
        .map_err(|e| Error::Tune(format!("built-in general set: {e}")))?;
    let tmp = tempfile::tempdir().map_err(|e| Error::Tune(e.to_string()))?;
    let root = tmp.path().join("general");
    for d in &set.docs {
        let p = root.join(&d.path);
        std::fs::create_dir_all(p.parent().unwrap_or(&root))
            .map_err(|e| Error::Tune(e.to_string()))?;
        std::fs::write(&p, &d.content).map_err(|e| Error::Tune(e.to_string()))?;
    }
    let general = eval_models(
        &EvalRequest {
            root: &root,
            state_dir: StateDir::Path(tmp.path().join("state")),
            vector_cache: None,
            models: &models,
            questions: &set.questions,
            source: &set.source,
            scope: &Scope::all(),
        },
        progress,
    )?;
    let gate = decide(&run.plan().id, &report, &general);
    let path = run.dir().join("eval.json");
    std::fs::write(
        &path,
        serde_json::to_string_pretty(
            &serde_json::json!({"gate": gate, "questions": report, "general": general}),
        )
        .unwrap_or_default(),
    )
    .map_err(|e| Error::Tune(format!("{}: {e}", path.display())))?;
    Ok(gate)
}

/// The gate recorded by the last `tune eval` of a run.
pub fn recorded_gate(run: &Run) -> Option<Gate> {
    let text = std::fs::read_to_string(run.dir().join("eval.json")).ok()?;
    let v: serde_json::Value = serde_json::from_str(&text).ok()?;
    serde_json::from_value(v.get("gate")?.clone()).ok()
}

fn previous_path(state: &Path) -> PathBuf {
    okbase_tune::runs_dir(state).join("previous-embed.json")
}

/// Switches the bundle to the run's tuned model (writes `okbase.toml`), remembering the current
/// setting for [`rollback`]. Refuses unless the recorded gate passed or `force`.
pub fn activate(bundle: &Bundle, run: &Run, force: bool) -> Result<String, Error> {
    let tuned = exported_model(run)
        .ok_or_else(|| Error::Tune("no exported model: run `okbase embed tune export`".into()))?;
    match recorded_gate(run) {
        Some(g) if g.passed && g.tuned == tuned => {}
        Some(g) if !force => {
            return Err(Error::Tune(format!(
                "the gate {} for {}; activation needs a passing `okbase embed tune eval` (or --force with the user's consent)",
                if g.passed {
                    "is for another model"
                } else {
                    "failed"
                },
                tuned
            )));
        }
        None if !force => {
            return Err(Error::Tune(
                "not evaluated: run `okbase embed tune eval` first".into(),
            ));
        }
        _ => {}
    }
    let state = bundle.state_path()?;
    let current = crate::config::load(bundle.root())?.embed;
    let file = bundle.root().join(crate::config::CONFIG_FILE);
    let before = std::fs::read_to_string(&file).ok();
    crate::config::write_embed(
        bundle.root(),
        &crate::config::EmbedConfig::Local {
            model: tuned.clone(),
        },
    )?;
    let after = std::fs::read_to_string(&file).unwrap_or_default();
    let saved = serde_json::json!({"config": current, "file_before": before, "file_after": after});
    let path = previous_path(&state);
    std::fs::create_dir_all(path.parent().unwrap_or(&state))
        .map_err(|e| Error::Tune(e.to_string()))?;
    std::fs::write(&path, saved.to_string())
        .map_err(|e| Error::Tune(format!("{}: {e}", path.display())))?;
    Ok(tuned)
}

/// Restores the embedding setting saved by the last [`activate`]; returns it.
pub fn rollback(bundle: &Bundle) -> Result<crate::config::EmbedConfig, Error> {
    let path = previous_path(&bundle.state_path()?);
    let text = std::fs::read_to_string(&path).map_err(|_| {
        Error::Tune(
            "nothing to roll back: no model was activated by `okbase embed tune activate`".into(),
        )
    })?;
    let saved: serde_json::Value =
        serde_json::from_str(&text).map_err(|e| Error::Tune(format!("{}: {e}", path.display())))?;
    let prev: crate::config::EmbedConfig = serde_json::from_value(saved["config"].clone())
        .map_err(|e| Error::Tune(format!("{}: {e}", path.display())))?;
    let file = bundle.root().join(crate::config::CONFIG_FILE);
    let now = std::fs::read_to_string(&file).ok();
    if now.as_deref() == saved["file_after"].as_str() {
        // Untouched since activate: restore the file exactly (or remove it if it did not exist).
        match saved["file_before"].as_str() {
            Some(before) => std::fs::write(&file, before),
            None => std::fs::remove_file(&file),
        }
        .map_err(|e| Error::Tune(format!("{}: {e}", file.display())))?;
    } else {
        crate::config::write_embed(bundle.root(), &prev)?;
    }
    let _ = std::fs::remove_file(&path);
    Ok(prev)
}

#[cfg(test)]
mod gate_tests {
    use super::*;

    fn score(model: &str, r1: f64, cross: Option<f64>) -> ModelScore {
        ModelScore {
            model: model.into(),
            n: 100,
            r_at_1: r1,
            r_at_3: r1,
            mrr: r1,
            cross_r_at_1: cross,
            cross_n: 50,
            same_r_at_1: cross,
            embed_seconds: 0.0,
        }
    }

    fn report(b: ModelScore, t: ModelScore) -> EvalReport {
        EvalReport {
            questions: "q".into(),
            models: vec![b, t],
        }
    }

    #[test]
    fn gate_needs_gain_no_cross_drop_no_regression() {
        let general = report(score("b", 0.85, None), score("t", 0.86, None));
        let ok = decide(
            "r",
            &report(score("b", 0.85, Some(0.8)), score("t", 0.9, Some(0.9))),
            &general,
        );
        assert!(ok.passed, "{:?}", ok.reasons);
        let small = decide(
            "r",
            &report(score("b", 0.85, Some(0.8)), score("t", 0.86, Some(0.9))),
            &general,
        );
        assert!(!small.passed && small.reasons[0].contains("too small"));
        let cross = decide(
            "r",
            &report(score("b", 0.85, Some(0.8)), score("t", 0.9, Some(0.7))),
            &general,
        );
        assert!(!cross.passed && cross.reasons[1].contains("dropped"));
        let worse = report(score("b", 0.85, None), score("t", 0.8, None));
        let reg = decide(
            "r",
            &report(score("b", 0.85, None), score("t", 0.9, None)),
            &worse,
        );
        assert!(!reg.passed && reg.reasons.last().unwrap().contains("regressed"));
        assert!(ok.to_text().contains("PASSED"));
    }

    #[test]
    fn regression_set_matches_the_fixture() {
        let set: RegressionSet = serde_json::from_str(REGRESSION).unwrap();
        assert_eq!((set.docs.len(), set.questions.len()), (100, 300));
        let fixture = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/multilingual");
        for d in &set.docs {
            assert_eq!(
                std::fs::read_to_string(fixture.join(&d.path)).unwrap(),
                d.content,
                "{}",
                d.path
            );
        }
    }
}
