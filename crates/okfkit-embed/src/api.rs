//! An OpenAI-compatible embeddings API (`POST {base_url}/embeddings`).
//! The API key is read from an environment variable, never from `okfkit.toml`.

use std::time::Duration;

use serde_json::{Value, json};

use crate::{Embedder, Error, normalize};

/// API settings.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ApiConfig {
    /// Base URL, such as `https://api.openai.com/v1`.
    pub base_url: String,
    /// Model name, such as `text-embedding-3-small`.
    pub model: String,
    /// Name of the environment variable holding the API key (`OPENAI_API_KEY`).
    pub key_env: String,
    /// Optional prefix for queries (some models want one).
    pub query_prefix: String,
}

/// An embeddings API client.
#[derive(Debug)]
pub struct ApiEmbedder {
    config: ApiConfig,
    id: String,
    key: String,
    agent: ureq::Agent,
}

impl ApiEmbedder {
    /// Creates a client. Fails if the key variable is not set.
    pub fn new(config: ApiConfig) -> Result<Self, Error> {
        let key = std::env::var(&config.key_env).unwrap_or_default();
        if key.is_empty() {
            return Err(Error::Api(format!(
                "environment variable {} (the API key) is not set",
                config.key_env
            )));
        }
        Ok(Self::with_key(config, key))
    }

    fn with_key(config: ApiConfig, key: String) -> Self {
        let agent: ureq::Agent = ureq::Agent::config_builder()
            .timeout_global(Some(Duration::from_secs(120)))
            .build()
            .into();
        ApiEmbedder {
            id: format!("api:{}", config.model),
            config,
            key,
            agent,
        }
    }

    fn call(&self, inputs: Vec<String>) -> Result<Vec<Vec<f32>>, Error> {
        let key = &self.key;
        let url = format!("{}/embeddings", self.config.base_url.trim_end_matches('/'));
        let mut out = Vec::with_capacity(inputs.len());
        for batch in inputs.chunks(64) {
            let resp: Value = self
                .agent
                .post(&url)
                .header("Authorization", &format!("Bearer {key}"))
                .send_json(json!({"model": self.config.model, "input": batch}))
                .map_err(|e| Error::Api(format!("{url}: {e}")))?
                .into_body()
                .read_json()
                .map_err(|e| Error::Api(format!("{url}: invalid response: {e}")))?;
            let mut data: Vec<(usize, Vec<f32>)> = resp["data"]
                .as_array()
                .ok_or_else(|| Error::Api(format!("{url}: response has no `data`")))?
                .iter()
                .enumerate()
                .map(|(i, d)| {
                    let idx = d["index"].as_u64().map_or(i, |x| x as usize);
                    let v = d["embedding"]
                        .as_array()
                        .map(|a| {
                            a.iter()
                                .filter_map(|x| x.as_f64())
                                .map(|x| x as f32)
                                .collect()
                        })
                        .unwrap_or_default();
                    (idx, v)
                })
                .collect();
            if data.len() != batch.len() {
                return Err(Error::Api(format!(
                    "{url}: expected {} embeddings, got {}",
                    batch.len(),
                    data.len()
                )));
            }
            data.sort_by_key(|d| d.0);
            out.extend(data.into_iter().map(|d| normalize(d.1)));
        }
        Ok(out)
    }
}

impl Embedder for ApiEmbedder {
    fn model_id(&self) -> &str {
        &self.id
    }

    fn embed_queries(&self, queries: &[String]) -> Result<Vec<Vec<f32>>, Error> {
        self.call(
            queries
                .iter()
                .map(|q| format!("{}{q}", self.config.query_prefix))
                .collect(),
        )
    }

    fn embed_documents(&self, docs: &[(String, String)]) -> Result<Vec<Vec<f32>>, Error> {
        self.call(
            docs.iter()
                .map(|(t, x)| {
                    if t.is_empty() {
                        x.clone()
                    } else {
                        format!("{t}\n{x}")
                    }
                })
                .collect(),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{BufRead, BufReader, Read, Write};

    /// Serves one OpenAI-style response per request on a local port.
    fn fake_server(responses: usize) -> String {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        std::thread::spawn(move || {
            for stream in listener.incoming().take(responses) {
                let mut s = stream.unwrap();
                let mut reader = BufReader::new(s.try_clone().unwrap());
                let mut len = 0;
                let mut auth = String::new();
                loop {
                    let mut line = String::new();
                    reader.read_line(&mut line).unwrap();
                    let l = line.to_ascii_lowercase();
                    if let Some(v) = l.strip_prefix("content-length:") {
                        len = v.trim().parse().unwrap();
                    }
                    if l.starts_with("authorization:") {
                        auth = line.trim().to_owned();
                    }
                    if line == "\r\n" {
                        break;
                    }
                }
                let mut body = vec![0; len];
                reader.read_exact(&mut body).unwrap();
                let req: Value = serde_json::from_slice(&body).unwrap();
                let n = req["input"].as_array().unwrap().len();
                assert!(auth.ends_with("Bearer sk-test"), "{auth}");
                // Return the items in reverse order to check index handling.
                let data: Vec<Value> = (0..n)
                    .rev()
                    .map(|i| json!({"index": i, "embedding": [i as f64 + 1.0, 0.0]}))
                    .collect();
                let out = json!({"data": data}).to_string();
                write!(s, "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{out}", out.len()).unwrap();
            }
        });
        format!("http://{addr}/v1")
    }

    #[test]
    fn calls_an_openai_compatible_endpoint() {
        let url = fake_server(1);
        let config = ApiConfig {
            base_url: url,
            model: "m".into(),
            key_env: "OKFKIT_TEST_KEY".into(),
            query_prefix: String::new(),
        };
        let e = ApiEmbedder::with_key(config, "sk-test".into());
        assert_eq!(e.model_id(), "api:m");
        let v = e
            .embed_documents(&[
                ("a".into(), "x".into()),
                ("b".into(), "y".into()),
                ("".into(), "z".into()),
            ])
            .unwrap();
        assert_eq!(v, [vec![1.0, 0.0], vec![1.0, 0.0], vec![1.0, 0.0]]);
        assert!(matches!(
            ApiEmbedder::new(ApiConfig {
                base_url: "x".into(),
                model: "m".into(),
                key_env: "OKFKIT_NO_SUCH_KEY".into(),
                query_prefix: String::new()
            }),
            Err(Error::Api(_))
        ));
    }
}
