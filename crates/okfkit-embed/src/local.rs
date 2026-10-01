//! Local ONNX models through fastembed.

use std::path::{Path, PathBuf};
use std::sync::Mutex;

use fastembed::{
    EmbeddingModel, InitOptionsUserDefined, Pooling, TextEmbedding, TextInitOptions,
    TokenizerFiles, UserDefinedEmbeddingModel,
};

use crate::models::{ModelInfo, Source};
use crate::{Embedder, Error, find_model, normalize};

const ACCEPTED: &str = "LICENSE-ACCEPTED";

/// A local model.
pub struct LocalEmbedder {
    info: &'static ModelInfo,
    model: Mutex<TextEmbedding>,
}

impl std::fmt::Debug for LocalEmbedder {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LocalEmbedder")
            .field("model", &self.info.id)
            .finish()
    }
}

/// `<user cache>/okfkit/models`, or `OKFKIT_MODELS_DIR`.
pub fn models_dir() -> Result<PathBuf, Error> {
    if let Some(d) = std::env::var_os("OKFKIT_MODELS_DIR").filter(|v| !v.is_empty()) {
        return Ok(PathBuf::from(d));
    }
    okfkit_analyze::dict::user_cache_dir()
        .map(|c| c.join("okfkit").join("models"))
        .ok_or_else(|| {
            Error::Model("no user cache directory (set HOME or OKFKIT_MODELS_DIR)".into())
        })
}

impl LocalEmbedder {
    /// Whether the license of `model` has been accepted (or needs no acceptance).
    pub fn license_accepted(info: &ModelInfo) -> bool {
        !info.requires_acceptance
            || models_dir().is_ok_and(|d| d.join(info.id).join(ACCEPTED).is_file())
    }

    /// Records that the user accepted the license of `model`.
    pub fn accept_license(info: &ModelInfo) -> Result<(), Error> {
        let dir = models_dir()?.join(info.id);
        std::fs::create_dir_all(&dir).map_err(|e| Error::Model(e.to_string()))?;
        std::fs::write(
            dir.join(ACCEPTED),
            format!("{}\n{}\n", info.license, info.license_url),
        )
        .map_err(|e| Error::Model(e.to_string()))
    }

    /// Loads (downloading on first use) a model. Fails if its license needs acceptance and was not accepted.
    pub fn load(model_id: &str, threads: Option<usize>) -> Result<Self, Error> {
        let info = find_model(model_id).ok_or_else(|| Error::UnknownModel(model_id.to_owned()))?;
        if !Self::license_accepted(info) {
            return Err(Error::LicenseNotAccepted {
                model: info.id.into(),
                license: info.license.into(),
                url: info.license_url.into(),
            });
        }
        let dir = models_dir()?;
        let threads =
            threads.unwrap_or_else(|| std::thread::available_parallelism().map_or(4, |n| n.get()));
        let model = match info.source {
            Source::Fastembed => {
                let which = match info.id {
                    "embeddinggemma-300m-q4" => EmbeddingModel::EmbeddingGemma300MQ4,
                    other => return Err(Error::UnknownModel(other.into())),
                };
                let opts = TextInitOptions::new(which)
                    .with_cache_dir(dir.join("fastembed"))
                    .with_max_length(512)
                    .with_intra_threads(threads)
                    .with_show_download_progress(false);
                TextEmbedding::try_new(opts).map_err(|e| Error::Model(e.to_string()))?
            }
            Source::HuggingFace { repo, onnx } => {
                let mdir = dir.join(info.id);
                download_hf(repo, onnx, &mdir)?;
                let rd = |f: &str| {
                    std::fs::read(mdir.join(f))
                        .map_err(|e| Error::Model(format!("{}/{f}: {e}", mdir.display())))
                };
                let tok = TokenizerFiles {
                    tokenizer_file: rd("tokenizer.json")?,
                    config_file: rd("config.json")?,
                    special_tokens_map_file: rd("special_tokens_map.json")?,
                    tokenizer_config_file: rd("tokenizer_config.json")?,
                };
                let model = UserDefinedEmbeddingModel::new(rd("model.onnx")?, tok)
                    .with_pooling(Pooling::Cls);
                let opts = InitOptionsUserDefined::new()
                    .with_max_length(512)
                    .with_intra_threads(threads);
                TextEmbedding::try_new_from_user_defined(model, opts)
                    .map_err(|e| Error::Model(e.to_string()))?
            }
        };
        Ok(LocalEmbedder {
            info,
            model: Mutex::new(model),
        })
    }

    /// The model description.
    pub fn info(&self) -> &'static ModelInfo {
        self.info
    }

    fn run(&self, texts: Vec<String>) -> Result<Vec<Vec<f32>>, Error> {
        let mut m = self.model.lock().unwrap_or_else(|e| e.into_inner());
        let vecs = m
            .embed(&texts, Some(16))
            .map_err(|e| Error::Model(e.to_string()))?;
        Ok(vecs.into_iter().map(normalize).collect())
    }
}

impl Embedder for LocalEmbedder {
    fn model_id(&self) -> &str {
        self.info.id
    }

    fn embed_queries(&self, queries: &[String]) -> Result<Vec<Vec<f32>>, Error> {
        self.run(
            queries
                .iter()
                .map(|q| self.info.prompting.query(q))
                .collect(),
        )
    }

    fn embed_documents(&self, docs: &[(String, String)]) -> Result<Vec<Vec<f32>>, Error> {
        self.run(
            docs.iter()
                .map(|(t, x)| self.info.prompting.document(t, x))
                .collect(),
        )
    }
}

/// Downloads the tokenizer files and the ONNX model of a Hugging Face repository into `dir` (once).
fn download_hf(repo: &str, onnx: &str, dir: &Path) -> Result<(), Error> {
    use std::io::Read;
    std::fs::create_dir_all(dir).map_err(|e| Error::Model(e.to_string()))?;
    let files = [
        ("tokenizer.json", "tokenizer.json", true),
        ("config.json", "config.json", false),
        ("special_tokens_map.json", "special_tokens_map.json", false),
        ("tokenizer_config.json", "tokenizer_config.json", false),
        (onnx, "model.onnx", true),
    ];
    for (remote, local, required) in files {
        let target = dir.join(local);
        if target.is_file() && target.metadata().is_ok_and(|m| m.len() > 0) {
            continue;
        }
        let url = format!("https://huggingface.co/{repo}/resolve/main/{remote}");
        let resp = ureq::get(&url).call();
        let mut bytes = Vec::new();
        match resp {
            Ok(r) => {
                r.into_body()
                    .as_reader()
                    .take(4 << 30)
                    .read_to_end(&mut bytes)
                    .map_err(|e| Error::Model(format!("{url}: {e}")))?;
            }
            Err(e) if required => return Err(Error::Model(format!("download {url}: {e}"))),
            Err(_) => bytes = b"{}".to_vec(), // optional files: fastembed accepts empty JSON
        }
        let tmp = dir.join(format!(".{local}.part"));
        std::fs::write(&tmp, &bytes).map_err(|e| Error::Model(e.to_string()))?;
        std::fs::rename(&tmp, &target).map_err(|e| Error::Model(e.to_string()))?;
    }
    Ok(())
}
