//! The Japanese dictionary (mecab-ipadic, via lindera).
//!
//! It is not embedded in the default build (it is ~58 MB). On first use it is
//! downloaded from the lindera project (checksum-verified, over rustls), built
//! once into the user cache, and loaded from there. Set `OKFKIT_OFFLINE=1` to
//! forbid downloads; Japanese text then falls back to character bigrams.

use std::path::{Path, PathBuf};

/// Where the source archive comes from (the same file and checksum lindera's own build uses).
pub const IPADIC_URL: &str = "https://Lindera.dev/mecab-ipadic-2.7.0-20250920.tar.gz";
/// MD5 of the archive, as pinned by `lindera-ipadic`.
pub const IPADIC_MD5: &str = "a95c409f12f1023fce8ef91f991ef042";
const IPADIC_SRC_DIR: &str = "mecab-ipadic-2.7.0-20250920";
const METADATA: &str = include_str!("../assets/ipadic-metadata.json");
/// Bumped when the built layout changes (also keyed on the lindera version).
const LAYOUT: &str = "ipadic-2.7.0-20250920-lindera6";
const READY: &str = ".okfkit-ready";

/// Errors installing the dictionary.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum DictError {
    /// Downloads are disabled (`OKFKIT_OFFLINE`) or no cache directory is known.
    #[error("{0}")]
    Unavailable(String),
    /// Download, verification, extraction or build failed.
    #[error("installing the Japanese dictionary failed: {0}")]
    Install(String),
}

/// The user cache directory (`$XDG_CACHE_HOME`, `~/.cache`, `~/Library/Caches`, `%LOCALAPPDATA%`).
pub fn user_cache_dir() -> Option<PathBuf> {
    let env = |k: &str| {
        std::env::var_os(k)
            .filter(|v| !v.is_empty())
            .map(PathBuf::from)
    };
    if cfg!(windows) {
        env("LOCALAPPDATA")
    } else if cfg!(target_os = "macos") {
        env("HOME").map(|h| h.join("Library/Caches"))
    } else {
        env("XDG_CACHE_HOME").or_else(|| env("HOME").map(|h| h.join(".cache")))
    }
}

/// Directory of the built dictionary: `OKFKIT_DICT_DIR`, else `<user cache>/okfkit/dict/<layout>`.
pub fn dictionary_dir() -> Option<PathBuf> {
    if let Some(d) = std::env::var_os("OKFKIT_DICT_DIR").filter(|v| !v.is_empty()) {
        return Some(PathBuf::from(d));
    }
    user_cache_dir().map(|c| c.join("okfkit").join("dict").join(LAYOUT))
}

/// Whether the dictionary is built and ready in [`dictionary_dir`].
pub fn installed() -> bool {
    dictionary_dir().is_some_and(|d| d.join(READY).is_file())
}

/// Whether okfkit may download the dictionary (`OKFKIT_OFFLINE` unset or `0`).
pub fn downloads_allowed() -> bool {
    cfg!(feature = "ja-download")
        && std::env::var("OKFKIT_OFFLINE").map_or(true, |v| v.is_empty() || v == "0")
}

/// Downloads, verifies and builds the dictionary into [`dictionary_dir`] (no-op if installed).
pub fn install() -> Result<PathBuf, DictError> {
    let dir = dictionary_dir().ok_or_else(|| {
        DictError::Unavailable("no user cache directory (set HOME or OKFKIT_DICT_DIR)".into())
    })?;
    if dir.join(READY).is_file() {
        return Ok(dir);
    }
    if !downloads_allowed() {
        return Err(DictError::Unavailable(
            "the Japanese dictionary is not installed and downloads are disabled (OKFKIT_OFFLINE)"
                .into(),
        ));
    }
    install_into(&dir)?;
    Ok(dir)
}

#[cfg(feature = "ja-download")]
fn install_into(dir: &Path) -> Result<(), DictError> {
    use std::io::Read;

    let err = |m: String| DictError::Install(m);
    let parent = dir
        .parent()
        .ok_or_else(|| err("invalid dictionary directory".into()))?;
    std::fs::create_dir_all(parent).map_err(|e| err(format!("{}: {e}", parent.display())))?;
    let work = parent.join(format!(".build-{}-{}", LAYOUT, std::process::id()));
    let _ = std::fs::remove_dir_all(&work);
    std::fs::create_dir_all(&work).map_err(|e| err(e.to_string()))?;
    let result = (|| {
        let mut body = ureq::get(IPADIC_URL)
            .call()
            .map_err(|e| err(format!("download {IPADIC_URL}: {e}")))?
            .into_body();
        let mut bytes = Vec::new();
        body.as_reader()
            .take(200 << 20)
            .read_to_end(&mut bytes)
            .map_err(|e| err(format!("download: {e}")))?;
        let digest = format!("{:x}", md5::compute(&bytes));
        if digest != IPADIC_MD5 {
            return Err(err(format!(
                "checksum mismatch: expected {IPADIC_MD5}, got {digest}"
            )));
        }
        tar::Archive::new(flate2::read::GzDecoder::new(bytes.as_slice()))
            .unpack(&work)
            .map_err(|e| err(format!("extract: {e}")))?;
        let metadata = serde_json::from_str(METADATA).map_err(|e| err(format!("metadata: {e}")))?;
        let out = work.join("dict");
        std::fs::create_dir_all(&out).map_err(|e| err(e.to_string()))?;
        lindera::dictionary::DictionaryBuilder::new(metadata)
            .build_dictionary(&work.join(IPADIC_SRC_DIR), &out)
            .map_err(|e| err(format!("build: {e}")))?;
        std::fs::write(out.join(READY), format!("{IPADIC_URL}\n{IPADIC_MD5}\n"))
            .map_err(|e| err(e.to_string()))?;
        if dir.exists() {
            let _ = std::fs::remove_dir_all(dir);
        }
        std::fs::rename(&out, dir).map_err(|e| err(format!("{}: {e}", dir.display())))
    })();
    let _ = std::fs::remove_dir_all(&work);
    result
}

#[cfg(not(feature = "ja-download"))]
fn install_into(_dir: &Path) -> Result<(), DictError> {
    Err(DictError::Unavailable(
        "this build cannot download the Japanese dictionary".into(),
    ))
}
