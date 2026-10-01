//! `okfkit.toml` (PLAN §4.4): every key is optional; a missing file means defaults.
//! v0.3 reads the `[modules] embed` and `[embed]` settings.

use std::path::{Path, PathBuf};

use crate::Error;

/// File name of the bundle configuration.
pub const CONFIG_FILE: &str = "okfkit.toml";

/// Where embeddings come from.
#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum EmbedConfig {
    /// No embeddings (the default): lexical tools only.
    #[default]
    Off,
    /// A local model by id (`embeddinggemma-300m-q4`, `bge-m3-int8`).
    Local {
        /// Model id.
        model: String,
    },
    /// An OpenAI-compatible API.
    Api {
        /// Base URL (`https://api.openai.com/v1`).
        base_url: String,
        /// Model name.
        model: String,
        /// Environment variable with the API key.
        key_env: String,
    },
}

/// The parts of `okfkit.toml` okfkit uses.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Config {
    /// Embedding settings.
    pub embed: EmbedConfig,
}

fn bad(path: &Path, msg: impl std::fmt::Display) -> Error {
    Error::Config(format!("{}: {msg}", path.display()))
}

/// Reads `<root>/okfkit.toml` (defaults if it does not exist).
pub fn load(root: &Path) -> Result<Config, Error> {
    let path = root.join(CONFIG_FILE);
    let Ok(text) = std::fs::read_to_string(&path) else {
        return Ok(Config::default());
    };
    let doc: toml_edit::DocumentMut = text.parse().map_err(|e| bad(&path, e))?;
    let get = |table: &str, key: &str| {
        doc.get(table)
            .and_then(|t| t.get(key))
            .and_then(|v| v.as_str())
            .map(str::to_owned)
    };
    let mode = get("modules", "embed").unwrap_or_else(|| "off".into());
    let embed = match mode.as_str() {
        "off" | "" => EmbedConfig::Off,
        "local" => EmbedConfig::Local {
            model: get("embed", "model").unwrap_or_else(|| okfkit_embed::models()[0].id.to_owned()),
        },
        "api" => {
            let api = doc.get("embed").and_then(|t| t.get("api"));
            let field = |k: &str| {
                api.and_then(|t| t.get(k))
                    .and_then(|v| v.as_str())
                    .map(str::to_owned)
            };
            EmbedConfig::Api {
                base_url: field("base_url").ok_or_else(|| {
                    bad(
                        &path,
                        "[embed.api] base_url is required when embed = \"api\"",
                    )
                })?,
                model: field("model").ok_or_else(|| {
                    bad(&path, "[embed.api] model is required when embed = \"api\"")
                })?,
                key_env: field("key_env").unwrap_or_else(|| "OPENAI_API_KEY".into()),
            }
        }
        other => {
            return Err(bad(
                &path,
                format!("[modules] embed must be off, local or api, not {other:?}"),
            ));
        }
    };
    Ok(Config { embed })
}

/// Writes the embedding settings into `<root>/okfkit.toml`, keeping the rest of the file. **Writes a file.**
pub fn write_embed(root: &Path, embed: &EmbedConfig) -> Result<PathBuf, Error> {
    let path = root.join(CONFIG_FILE);
    let text = std::fs::read_to_string(&path).unwrap_or_default();
    let mut doc: toml_edit::DocumentMut = text.parse().map_err(|e| bad(&path, e))?;
    let table = |doc: &mut toml_edit::DocumentMut, name: &str| -> Result<(), Error> {
        if doc.get(name).is_none() {
            doc[name] = toml_edit::table();
        }
        doc[name]
            .as_table_mut()
            .map(|_| ())
            .ok_or_else(|| bad(&path, format!("[{name}] is not a table")))
    };
    table(&mut doc, "modules")?;
    let mode = match embed {
        EmbedConfig::Off => "off",
        EmbedConfig::Local { .. } => "local",
        EmbedConfig::Api { .. } => "api",
    };
    doc["modules"]["embed"] = toml_edit::value(mode);
    match embed {
        EmbedConfig::Off => {}
        EmbedConfig::Local { model } => {
            table(&mut doc, "embed")?;
            doc["embed"]["model"] = toml_edit::value(model.as_str());
        }
        EmbedConfig::Api {
            base_url,
            model,
            key_env,
        } => {
            table(&mut doc, "embed")?;
            let mut api = toml_edit::Table::new();
            api["base_url"] = toml_edit::value(base_url.as_str());
            api["model"] = toml_edit::value(model.as_str());
            api["key_env"] = toml_edit::value(key_env.as_str());
            doc["embed"]["api"] = toml_edit::Item::Table(api);
        }
    }
    std::fs::write(&path, doc.to_string()).map_err(|e| bad(&path, e))?;
    Ok(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn read_and_write() {
        let tmp = tempfile::tempdir().unwrap();
        assert_eq!(load(tmp.path()).unwrap(), Config::default());
        std::fs::write(
            tmp.path().join(CONFIG_FILE),
            "# my settings\nprofile = \"standard\"\n",
        )
        .unwrap();
        write_embed(
            tmp.path(),
            &EmbedConfig::Local {
                model: "bge-m3-int8".into(),
            },
        )
        .unwrap();
        let text = std::fs::read_to_string(tmp.path().join(CONFIG_FILE)).unwrap();
        assert!(
            text.starts_with("# my settings\nprofile = \"standard\"\n"),
            "{text}"
        );
        assert_eq!(
            load(tmp.path()).unwrap().embed,
            EmbedConfig::Local {
                model: "bge-m3-int8".into()
            }
        );
        let api = EmbedConfig::Api {
            base_url: "https://x/v1".into(),
            model: "m".into(),
            key_env: "K".into(),
        };
        write_embed(tmp.path(), &api).unwrap();
        assert_eq!(load(tmp.path()).unwrap().embed, api);
        std::fs::write(tmp.path().join(CONFIG_FILE), "[modules]\nembed = \"gpu\"\n").unwrap();
        assert!(load(tmp.path()).is_err());
    }
}
