//! Fine-tuning support in the facade: runs live in the bundle's state dir, and
//! [`eval_models`] measures retrieval of several models on the same questions
//! (docs/PLAN-advise-tune.md §2).

use std::collections::{BTreeMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use serde::{Deserialize, Serialize};

use crate::{Bundle, Error, OpenOptions, Scope, SearchRequest, StateDir};

pub use okfkit_tune::{
    BatchView, GUIDE, Kind, Plan, Run, STANDARD, Settings, Status as TuneStatus, Submission,
};

/// Human-written evaluation questions, preferred over the generated held-out set.
pub const HUMAN_EVAL_PATH: &str = "_meta/eval/questions.jsonl";

impl From<okfkit_tune::Error> for Error {
    fn from(e: okfkit_tune::Error) -> Self {
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
        let langs = okfkit_query::content_langs(&self.index(), scope)?;
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
        Ok(okfkit_tune::init(&self.index(), scope, &state, settings)?)
    }

    /// Opens a run (`None`: the newest).
    pub fn tune_run(&self, id: Option<&str>) -> Result<Run, Error> {
        Ok(Run::find(&self.state_path()?, id)?)
    }

    /// Run ids, oldest first.
    pub fn tune_runs(&self) -> Result<Vec<String>, Error> {
        Ok(okfkit_tune::runs(&self.state_path()?))
    }

    /// Normalized human-written eval questions (must not be reused for training).
    pub fn human_eval_questions(&self) -> Result<HashSet<String>, Error> {
        let path = self.root().join(HUMAN_EVAL_PATH);
        if !path.is_file() {
            return Ok(HashSet::new());
        }
        Ok(load_questions(&path)?
            .iter()
            .map(|q| okfkit_tune::normalize(&q.q))
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
