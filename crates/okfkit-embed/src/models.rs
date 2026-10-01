//! The supported local models and how they format inputs (from spike S1).

/// How text is fed to a model.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Prompting {
    /// EmbeddingGemma: `task: search result | query: …` and `title: … | text: …`.
    Gemma,
    /// No prompts; documents are `title\ntext`.
    Plain,
}

impl Prompting {
    /// The text embedded for a query.
    pub fn query(self, q: &str) -> String {
        match self {
            Prompting::Gemma => format!("task: search result | query: {q}"),
            Prompting::Plain => q.to_owned(),
        }
    }

    /// The text embedded for a document chunk.
    pub fn document(self, title: &str, text: &str) -> String {
        match self {
            Prompting::Gemma => format!("title: {title} | text: {text}"),
            Prompting::Plain if title.is_empty() => text.to_owned(),
            Prompting::Plain => format!("{title}\n{text}"),
        }
    }
}

/// Where a model's files come from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Source {
    /// A model fastembed knows (downloaded by fastembed from Hugging Face).
    Fastembed,
    /// An ONNX export on Hugging Face: repository and path of the `.onnx` file (CLS pooling).
    HuggingFace {
        /// Repository, such as `Xenova/bge-m3`.
        repo: &'static str,
        /// Path of the model file in the repository.
        onnx: &'static str,
    },
}

/// A supported local model.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ModelInfo {
    /// Id used in `okfkit.toml` and on the command line.
    pub id: &'static str,
    /// Display name.
    pub name: &'static str,
    /// License name.
    pub license: &'static str,
    /// License URL.
    pub license_url: &'static str,
    /// Whether the user must accept the license before the download.
    pub requires_acceptance: bool,
    /// Vector dimension.
    pub dim: usize,
    /// Approximate download size in MB.
    pub size_mb: usize,
    /// Input formatting.
    pub prompting: Prompting,
    /// Where the files come from.
    pub source: Source,
    /// Retrieval quality in spike S1 (R@1 on 300 vi/en/ja questions), for `okfkit embed models`.
    pub s1_r_at_1: f32,
    /// Longest input in model tokens (longer inputs are truncated).
    pub max_length: usize,
}

const MODELS: &[ModelInfo] = &[
    ModelInfo {
        id: "embeddinggemma-300m-q4",
        name: "EmbeddingGemma 300M (Q4)",
        license: "Gemma Terms of Use",
        license_url: "https://ai.google.dev/gemma/terms",
        requires_acceptance: true,
        dim: 768,
        size_mb: 188,
        prompting: Prompting::Gemma,
        source: Source::Fastembed,
        s1_r_at_1: 0.847,
        max_length: 2048,
    },
    ModelInfo {
        id: "embeddinggemma-300m",
        name: "EmbeddingGemma 300M (fp32)",
        license: "Gemma Terms of Use",
        license_url: "https://ai.google.dev/gemma/terms",
        requires_acceptance: true,
        dim: 768,
        size_mb: 1230,
        prompting: Prompting::Gemma,
        source: Source::Fastembed,
        s1_r_at_1: 0.863,
        max_length: 2048,
    },
    ModelInfo {
        id: "bge-m3-int8",
        name: "BAAI bge-m3 (int8)",
        license: "MIT",
        license_url: "https://huggingface.co/BAAI/bge-m3",
        requires_acceptance: false,
        dim: 1024,
        size_mb: 570,
        prompting: Prompting::Plain,
        source: Source::HuggingFace {
            repo: "Xenova/bge-m3",
            onnx: "onnx/model_int8.onnx",
        },
        s1_r_at_1: 0.780,
        max_length: 2048,
    },
];

const ACCEPTED: &str = "LICENSE-ACCEPTED";

#[cfg(test)]
thread_local! {
    /// Per-test models directory (tests must not mutate the process environment).
    pub(crate) static MODELS_DIR_OVERRIDE: std::cell::RefCell<Option<std::path::PathBuf>> =
        const { std::cell::RefCell::new(None) };
}

/// `<user cache>/okfkit/models`, or `OKFKIT_MODELS_DIR`.
pub fn models_dir() -> Result<std::path::PathBuf, crate::Error> {
    #[cfg(test)]
    if let Some(d) = MODELS_DIR_OVERRIDE.with(|o| o.borrow().clone()) {
        return Ok(d);
    }
    if let Some(d) = std::env::var_os("OKFKIT_MODELS_DIR").filter(|v| !v.is_empty()) {
        return Ok(d.into());
    }
    okfkit_analyze::dict::user_cache_dir()
        .map(|c| c.join("okfkit").join("models"))
        .ok_or_else(|| {
            crate::Error::Model("no user cache directory (set HOME or OKFKIT_MODELS_DIR)".into())
        })
}

/// Licenses are accepted once for all models under them (`licenses/<name>/LICENSE-ACCEPTED`).
fn license_dir(info: &ModelInfo) -> Result<std::path::PathBuf, crate::Error> {
    let slug: String = info
        .license
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() {
                c.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .collect();
    Ok(models_dir()?.join("licenses").join(slug))
}

/// Whether the license of a model has been accepted (or needs no acceptance).
pub fn license_accepted(info: &ModelInfo) -> bool {
    !info.requires_acceptance
        || license_dir(info).is_ok_and(|d| d.join(ACCEPTED).is_file())
        // Acceptances recorded per model by earlier builds.
        || models_dir().is_ok_and(|d| d.join(info.id).join(ACCEPTED).is_file())
}

/// Records that the user accepted the license of a model (for every model under that license).
pub fn accept_license(info: &ModelInfo) -> Result<(), crate::Error> {
    let dir = license_dir(info)?;
    std::fs::create_dir_all(&dir).map_err(|e| crate::Error::Model(e.to_string()))?;
    std::fs::write(
        dir.join(ACCEPTED),
        format!("{}\n{}\n", info.license, info.license_url),
    )
    .map_err(|e| crate::Error::Model(e.to_string()))
}

/// The supported local models; the first is the default (best in spike S1).
pub fn models() -> &'static [ModelInfo] {
    MODELS
}

/// Looks up a model by id.
pub fn find_model(id: &str) -> Option<&'static ModelInfo> {
    MODELS.iter().find(|m| m.id == id)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prompts() {
        let g = find_model("embeddinggemma-300m-q4").unwrap();
        assert_eq!(
            g.prompting.query("đổi trả"),
            "task: search result | query: đổi trả"
        );
        assert_eq!(
            g.prompting.document("Refunds", "30 days"),
            "title: Refunds | text: 30 days"
        );
        assert_eq!(Prompting::Plain.document("", "x"), "x");
        assert!(g.requires_acceptance && !find_model("bge-m3-int8").unwrap().requires_acceptance);
    }
}
