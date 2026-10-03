//! Local ONNX models through fastembed.

use std::path::Path;
use std::sync::Mutex;

use fastembed::{
    EmbeddingModel, InitOptionsUserDefined, OutputKey, Pooling, TextEmbedding, TextInitOptions,
    TokenizerFiles, UserDefinedEmbeddingModel,
};

use crate::custom::{self, CustomModel};
use crate::models::{ModelInfo, Prompting, Source};
use crate::{Embedder, Error, find_model, normalize};

/// A local model.
pub struct LocalEmbedder {
    id: String,
    prompting: Prompting,
    info: Option<&'static ModelInfo>,
    model: Mutex<TextEmbedding>,
}

impl std::fmt::Debug for LocalEmbedder {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LocalEmbedder")
            .field("model", &self.id)
            .finish()
    }
}

impl LocalEmbedder {
    /// Loads (downloading on first use) a built-in model, or an installed `custom:<name>` model.
    /// Fails if its license needs acceptance and was not accepted.
    pub fn load(model_id: &str, threads: Option<usize>) -> Result<Self, Error> {
        let threads =
            threads.unwrap_or_else(|| std::thread::available_parallelism().map_or(4, |n| n.get()));
        if model_id.starts_with(custom::PREFIX) {
            return Self::load_custom(&custom::find_custom(model_id)?, threads);
        }
        let info = find_model(model_id).ok_or_else(|| Error::UnknownModel(model_id.to_owned()))?;
        check_license(info)?;
        let dir = crate::models::models_dir()?;
        let model = match info.source {
            Source::Fastembed => {
                let which = match info.id {
                    "embeddinggemma-300m-q4" => EmbeddingModel::EmbeddingGemma300MQ4,
                    "embeddinggemma-300m" => EmbeddingModel::EmbeddingGemma300M,
                    other => return Err(Error::UnknownModel(other.into())),
                };
                let opts = TextInitOptions::new(which)
                    .with_cache_dir(dir.join("fastembed"))
                    .with_max_length(info.max_length)
                    .with_intra_threads(threads)
                    .with_show_download_progress(false);
                TextEmbedding::try_new(opts).map_err(|e| Error::Model(e.to_string()))?
            }
            Source::HuggingFace { repo, onnx } => {
                let mdir = dir.join(info.id);
                download_hf(repo, onnx, &mdir)?;
                let model =
                    UserDefinedEmbeddingModel::new(read(&mdir, "model.onnx")?, tokenizer(&mdir)?)
                        .with_pooling(Pooling::Cls);
                user_defined(model, info.max_length, threads)?
            }
        };
        Ok(LocalEmbedder {
            id: info.id.to_owned(),
            prompting: info.prompting,
            info: Some(info),
            model: Mutex::new(model),
        })
    }

    fn load_custom(c: &CustomModel, threads: usize) -> Result<Self, Error> {
        let m = &c.manifest;
        if let Some(base) = m.base.as_deref().and_then(find_model) {
            check_license(base)?;
        }
        let mut model = UserDefinedEmbeddingModel::new(read(&c.dir, &m.onnx)?, tokenizer(&c.dir)?);
        for f in &m.external_data {
            let name = std::path::Path::new(f)
                .file_name()
                .map_or_else(|| f.clone(), |n| n.to_string_lossy().into_owned());
            model = model.with_external_initializer(name, read(&c.dir, f)?);
        }
        model = match m.pooling {
            custom::Pooling::Output => {
                model.output_key = Some(OutputKey::ByName("sentence_embedding"));
                model.with_pooling(Pooling::Mean)
            }
            custom::Pooling::Mean => model.with_pooling(Pooling::Mean),
            custom::Pooling::Cls => model.with_pooling(Pooling::Cls),
        };
        Ok(LocalEmbedder {
            id: c.model_id(),
            prompting: m.prompting(),
            info: None,
            model: Mutex::new(user_defined(model, m.max_length, threads)?),
        })
    }

    /// The built-in model description (`None` for custom models).
    pub fn info(&self) -> Option<&'static ModelInfo> {
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
        &self.id
    }

    fn embed_queries(&self, queries: &[String]) -> Result<Vec<Vec<f32>>, Error> {
        self.run(queries.iter().map(|q| self.prompting.query(q)).collect())
    }

    fn embed_documents(&self, docs: &[(String, String)]) -> Result<Vec<Vec<f32>>, Error> {
        self.run(
            docs.iter()
                .map(|(t, x)| self.prompting.document(t, x))
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

fn check_license(info: &ModelInfo) -> Result<(), Error> {
    if crate::models::license_accepted(info) {
        Ok(())
    } else {
        Err(Error::LicenseNotAccepted {
            model: info.id.into(),
            license: info.license.into(),
            url: info.license_url.into(),
        })
    }
}

fn read(dir: &Path, f: &str) -> Result<Vec<u8>, Error> {
    std::fs::read(dir.join(f)).map_err(|e| Error::Model(format!("{}/{f}: {e}", dir.display())))
}

fn tokenizer(dir: &Path) -> Result<TokenizerFiles, Error> {
    Ok(TokenizerFiles {
        tokenizer_file: read(dir, "tokenizer.json")?,
        config_file: read(dir, "config.json")?,
        special_tokens_map_file: read(dir, "special_tokens_map.json")?,
        tokenizer_config_file: read(dir, "tokenizer_config.json")?,
    })
}

fn user_defined(
    model: UserDefinedEmbeddingModel,
    max_length: usize,
    threads: usize,
) -> Result<TextEmbedding, Error> {
    let opts = InitOptionsUserDefined::new()
        .with_max_length(max_length)
        .with_intra_threads(threads);
    TextEmbedding::try_new_from_user_defined(model, opts).map_err(|e| Error::Model(e.to_string()))
}
