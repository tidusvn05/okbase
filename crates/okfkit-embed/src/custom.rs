//! User-supplied ONNX embedding models (`custom:<name>`), such as a model fine-tuned on a bundle.
//!
//! A model is a directory with an `okfkit-model.json` manifest, the ONNX file (plus external data
//! files) and the tokenizer files. `okfkit embed models add <dir>` copies it into
//! `<models dir>/custom/<name>/` and records a content hash. The hash is part of the model id
//! (`custom:<name>@<hash>`), so replacing the weights never reuses vectors of the old ones.
//! okfkit never downloads or distributes custom models.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::models::Prompting;
use crate::{Error, find_model, models_dir};

/// The manifest file name.
pub const MANIFEST: &str = "okfkit-model.json";
/// The `custom:` prefix of model ids.
pub const PREFIX: &str = "custom:";
/// Tokenizer files every model needs.
pub const TOKENIZER_FILES: [&str; 4] = [
    "tokenizer.json",
    "config.json",
    "special_tokens_map.json",
    "tokenizer_config.json",
];

/// How a model turns token states into one vector.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Pooling {
    /// The graph outputs `sentence_embedding` (pooling and dense layers inside the model).
    Output,
    /// Mean of the token states.
    Mean,
    /// The first token's state.
    Cls,
}

/// `okfkit-model.json`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Manifest {
    /// Manifest format version (1).
    pub format: u32,
    /// Name: lowercase letters, digits, `-`, `_`, `.`.
    pub name: String,
    /// One-line description.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub description: String,
    /// The built-in model this one derives from (its license applies), if any.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub base: Option<String>,
    /// License name.
    pub license: String,
    /// License URL.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub license_url: String,
    /// The ONNX file, relative to the directory.
    pub onnx: String,
    /// External data files of the ONNX graph.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub external_data: Vec<String>,
    /// Pooling.
    pub pooling: Pooling,
    /// Input prompts: `gemma` or `plain`.
    pub prompting: PromptingName,
    /// Vector dimension.
    pub dim: usize,
    /// Longest input in model tokens.
    pub max_length: usize,
    /// blake3 of the ONNX and data files, set on install.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hash: Option<String>,
    /// Free-form provenance (training data hash, metrics, …), written by `okfkit embed tune`.
    #[serde(default, skip_serializing_if = "serde_json::Value::is_null")]
    pub provenance: serde_json::Value,
}

/// Prompt scheme names in the manifest.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PromptingName {
    /// See [`Prompting::Gemma`].
    Gemma,
    /// See [`Prompting::Plain`].
    Plain,
}

impl Manifest {
    /// The prompt scheme.
    pub fn prompting(&self) -> Prompting {
        match self.prompting {
            PromptingName::Gemma => Prompting::Gemma,
            PromptingName::Plain => Prompting::Plain,
        }
    }

    /// Whether the user must accept the license first (inherited from the base model).
    pub fn requires_acceptance(&self) -> bool {
        self.base
            .as_deref()
            .and_then(find_model)
            .is_some_and(|b| b.requires_acceptance)
    }
}

/// An installed custom model.
#[derive(Debug, Clone, PartialEq)]
pub struct CustomModel {
    /// Directory.
    pub dir: PathBuf,
    /// Manifest.
    pub manifest: Manifest,
}

impl CustomModel {
    /// `custom:<name>@<hash prefix>`: the id vectors are stored under.
    pub fn model_id(&self) -> String {
        let hash = self.manifest.hash.as_deref().unwrap_or("unhashed");
        format!(
            "{PREFIX}{}@{}",
            self.manifest.name,
            &hash[..hash.len().min(12)]
        )
    }
}

fn err(msg: impl Into<String>) -> Error {
    Error::Model(msg.into())
}

/// `<models dir>/custom`.
pub fn custom_dir() -> Result<PathBuf, Error> {
    Ok(models_dir()?.join("custom"))
}

fn valid_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 64
        && !name.starts_with('.')
        && name
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || "-_.".contains(c))
}

/// Reads and checks a manifest and the files it names.
pub fn read_manifest(dir: &Path) -> Result<Manifest, Error> {
    let path = dir.join(MANIFEST);
    let text = std::fs::read_to_string(&path).map_err(|e| {
        err(format!(
            "{}: {e} (a custom model needs {MANIFEST})",
            path.display()
        ))
    })?;
    let m: Manifest =
        serde_json::from_str(&text).map_err(|e| err(format!("{}: {e}", path.display())))?;
    if m.format != 1 {
        return Err(err(format!(
            "{}: unsupported format {}",
            path.display(),
            m.format
        )));
    }
    if !valid_name(&m.name) {
        return Err(err(format!(
            "{}: invalid name `{}` (use lowercase letters, digits, - _ .)",
            path.display(),
            m.name
        )));
    }
    if let Some(base) = &m.base
        && find_model(base).is_none()
    {
        return Err(err(format!(
            "{}: unknown base model `{base}`",
            path.display()
        )));
    }
    for f in std::iter::once(&m.onnx)
        .chain(&m.external_data)
        .map(String::as_str)
        .chain(TOKENIZER_FILES)
    {
        if f.contains("..") || Path::new(f).is_absolute() || !dir.join(f).is_file() {
            return Err(err(format!(
                "{}: missing or invalid file `{f}`",
                dir.display()
            )));
        }
    }
    Ok(m)
}

fn hash_files(dir: &Path, m: &Manifest) -> Result<String, Error> {
    let mut h = blake3::Hasher::new();
    for f in std::iter::once(&m.onnx).chain(&m.external_data) {
        let mut file = std::fs::File::open(dir.join(f)).map_err(|e| err(format!("{f}: {e}")))?;
        h.update(f.as_bytes());
        std::io::copy(&mut file, &mut h).map_err(|e| err(format!("{f}: {e}")))?;
    }
    Ok(h.finalize().to_hex().to_string())
}

/// Installed custom models, sorted by name.
pub fn custom_models() -> Result<Vec<CustomModel>, Error> {
    let dir = custom_dir()?;
    let Ok(entries) = std::fs::read_dir(&dir) else {
        return Ok(Vec::new());
    };
    let mut out: Vec<CustomModel> = entries
        .flatten()
        .filter(|e| e.path().join(MANIFEST).is_file())
        .filter_map(|e| {
            let manifest = read_manifest(&e.path()).ok()?;
            Some(CustomModel {
                dir: e.path(),
                manifest,
            })
        })
        .collect();
    out.sort_by(|a, b| a.manifest.name.cmp(&b.manifest.name));
    Ok(out)
}

/// Looks up an installed model by `name`, `custom:name` or `custom:name@hash`.
pub fn find_custom(id: &str) -> Result<CustomModel, Error> {
    let name = id.strip_prefix(PREFIX).unwrap_or(id);
    let name = name.split('@').next().unwrap_or(name);
    let dir = custom_dir()?.join(name);
    if !valid_name(name) || !dir.join(MANIFEST).is_file() {
        return Err(Error::UnknownModel(format!("{PREFIX}{name}")));
    }
    let manifest = read_manifest(&dir)?;
    if manifest.name != name {
        return Err(err(format!(
            "{}: manifest name `{}` does not match the directory",
            dir.display(),
            manifest.name
        )));
    }
    Ok(CustomModel { dir, manifest })
}

/// Copies the model at `src` into the custom models directory (under `name`, else the manifest's)
/// and records its hash. Refuses to overwrite unless `replace`.
pub fn install_custom(src: &Path, name: Option<&str>, replace: bool) -> Result<CustomModel, Error> {
    let mut m = read_manifest(src)?;
    if let Some(n) = name {
        if !valid_name(n) {
            return Err(err(format!(
                "invalid name `{n}` (use lowercase letters, digits, - _ .)"
            )));
        }
        m.name = n.to_owned();
    }
    let root = custom_dir()?;
    let dest = root.join(&m.name);
    if dest.exists() && !replace {
        return Err(err(format!(
            "custom model `{}` exists; pass --replace to overwrite it",
            m.name
        )));
    }
    m.hash = Some(hash_files(src, &m)?);
    let tmp = root.join(format!(".{}.tmp", m.name));
    let _ = std::fs::remove_dir_all(&tmp);
    std::fs::create_dir_all(&tmp).map_err(|e| err(format!("{}: {e}", tmp.display())))?;
    for f in std::iter::once(&m.onnx)
        .chain(&m.external_data)
        .map(String::as_str)
        .chain(TOKENIZER_FILES)
    {
        let to = tmp.join(f);
        if let Some(parent) = to.parent() {
            std::fs::create_dir_all(parent).map_err(|e| err(e.to_string()))?;
        }
        std::fs::copy(src.join(f), &to).map_err(|e| err(format!("{f}: {e}")))?;
    }
    let json = serde_json::to_string_pretty(&m).map_err(|e| err(e.to_string()))?;
    std::fs::write(tmp.join(MANIFEST), json + "\n").map_err(|e| err(e.to_string()))?;
    if dest.exists() {
        std::fs::remove_dir_all(&dest).map_err(|e| err(format!("{}: {e}", dest.display())))?;
    }
    std::fs::rename(&tmp, &dest).map_err(|e| err(format!("{}: {e}", dest.display())))?;
    Ok(CustomModel {
        dir: dest,
        manifest: m,
    })
}

/// Deletes an installed custom model.
pub fn remove_custom(id: &str) -> Result<(), Error> {
    let m = find_custom(id)?;
    std::fs::remove_dir_all(&m.dir).map_err(|e| err(format!("{}: {e}", m.dir.display())))
}

/// The id vectors are stored under for a configured model: `custom:<name>@<hash>` for custom
/// models (reads the manifest only), the id itself otherwise.
pub fn resolve_model_id(model: &str) -> Result<String, Error> {
    if model.starts_with(PREFIX) {
        Ok(find_custom(model)?.model_id())
    } else {
        Ok(model.to_owned())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write_model(dir: &Path, name: &str, weights: &[u8]) {
        std::fs::create_dir_all(dir).unwrap();
        std::fs::write(dir.join("model.onnx"), weights).unwrap();
        std::fs::write(dir.join("model.onnx_data"), b"data").unwrap();
        for f in TOKENIZER_FILES {
            std::fs::write(dir.join(f), "{}").unwrap();
        }
        let m = serde_json::json!({
            "format": 1, "name": name, "base": "embeddinggemma-300m-q4",
            "license": "Gemma Terms of Use", "onnx": "model.onnx", "external_data": ["model.onnx_data"],
            "pooling": "output", "prompting": "gemma", "dim": 768, "max_length": 2048
        });
        std::fs::write(dir.join(MANIFEST), m.to_string()).unwrap();
    }

    #[test]
    fn install_find_hash_remove() {
        let tmp = tempfile::tempdir().unwrap();
        let models = tmp.path().join("models");
        let src = tmp.path().join("src");
        write_model(&src, "acme", b"weights-1");
        let installed = with_models_dir(&models, || install_custom(&src, None, false)).unwrap();
        assert!(installed.model_id().starts_with("custom:acme@"));
        assert!(installed.manifest.requires_acceptance());
        let again = with_models_dir(&models, || install_custom(&src, None, false));
        assert!(again.unwrap_err().to_string().contains("--replace"));
        // New weights, new id: old vectors are never reused.
        write_model(&src, "acme", b"weights-2");
        let replaced = with_models_dir(&models, || install_custom(&src, None, true)).unwrap();
        assert_ne!(replaced.model_id(), installed.model_id());
        let listed = with_models_dir(&models, custom_models).unwrap();
        assert_eq!(listed.len(), 1);
        assert_eq!(
            with_models_dir(&models, || resolve_model_id("custom:acme")).unwrap(),
            replaced.model_id()
        );
        assert_eq!(
            with_models_dir(&models, || resolve_model_id("bge-m3-int8")).unwrap(),
            "bge-m3-int8"
        );
        with_models_dir(&models, || remove_custom("acme")).unwrap();
        assert!(with_models_dir(&models, || find_custom("acme")).is_err());
        // Bad manifests are rejected with the reason.
        write_model(&src, "Bad Name", b"w");
        let e = with_models_dir(&models, || install_custom(&src, None, false)).unwrap_err();
        assert!(e.to_string().contains("invalid name"), "{e}");
        std::fs::remove_file(src.join("tokenizer.json")).unwrap();
        let e = read_manifest(&src).unwrap_err();
        assert!(e.to_string().contains("invalid name") || e.to_string().contains("tokenizer.json"));
    }

    fn with_models_dir<T>(dir: &Path, f: impl FnOnce() -> T) -> T {
        MODELS_DIR_OVERRIDE.with(|o| *o.borrow_mut() = Some(dir.to_owned()));
        let out = f();
        MODELS_DIR_OVERRIDE.with(|o| *o.borrow_mut() = None);
        out
    }

    use crate::models::MODELS_DIR_OVERRIDE;
}
