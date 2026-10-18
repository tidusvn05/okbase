//! Resolve Git templates without checkout, submodules, filters or executable hooks.

use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use super::{MAX_BYTES, MAX_FILES, Template, invalid, safe_path};
use crate::Error;

/// A disposable repository snapshot. Dropping it removes both objects and exported files.
pub struct Repository {
    temporary: tempfile::TempDir,
    /// Exact commit resolved from the requested branch, tag or commit.
    pub commit: String,
    url: String,
}

impl Repository {
    /// Fetch an explicitly selected HTTPS/SSH repository or local Git directory.
    /// Requires Git on PATH; credentials must already be configured externally.
    pub fn fetch(url: &str, reference: Option<&str>) -> Result<Self, Error> {
        let local = Path::new(url).is_dir();
        let ssh = regex::Regex::new(r"^(?:ssh://(?:[A-Za-z0-9_.-]+@)?[A-Za-z0-9.-]+(?::[0-9]+)?/[^\s]+|[A-Za-z0-9_.-]+@[A-Za-z0-9.-]+:[^\s]+)$").expect("constant regex").is_match(url);
        if !local {
            let https = url
                .strip_prefix("https://")
                .and_then(|u| u.split('/').next())
                .is_some_and(|a| !a.is_empty() && !a.contains('@'));
            if (!https && !ssh) || url.chars().any(char::is_control) || url.contains(['?', '#']) {
                return Err(invalid(
                    "template repository must be an HTTPS URL without embedded credentials, an SSH URL, or a local Git directory",
                ));
            }
            if std::env::var("OKBASE_OFFLINE").is_ok_and(|v| v == "1" || v == "true") {
                return Err(invalid(
                    "remote template fetch is disabled by OKBASE_OFFLINE; use --template-path",
                ));
            }
        }
        let reference = reference.unwrap_or("HEAD");
        if reference.is_empty()
            || reference.starts_with('-')
            || reference.contains([':', '\r', '\n', ' ', '\t'])
        {
            return Err(invalid("--ref must be a branch, tag or commit"));
        }
        let url = if local {
            Path::new(url)
                .canonicalize()
                .map_err(invalid)?
                .to_string_lossy()
                .into_owned()
        } else {
            url.to_owned()
        };
        let temporary = tempfile::tempdir().map_err(invalid)?;
        std::fs::create_dir(temporary.path().join("hooks")).map_err(invalid)?;
        std::fs::write(temporary.path().join("config"), "").map_err(invalid)?;
        let mut repo = Self {
            temporary,
            commit: String::new(),
            url,
        };
        repo.run(&["init", "--bare", "--template=", "objects"])?;
        repo.run(&[
            "--git-dir=objects",
            "-c",
            "protocol.allow=never",
            "-c",
            if local {
                "protocol.file.allow=always"
            } else if ssh {
                "protocol.ssh.allow=always"
            } else {
                "protocol.https.allow=always"
            },
            "fetch",
            "--quiet",
            "--depth=1",
            "--no-tags",
            "--no-recurse-submodules",
            "--",
            &repo.url,
            reference,
        ])?;
        repo.commit = String::from_utf8(repo.run(&[
            "--git-dir=objects",
            "rev-parse",
            "--verify",
            "FETCH_HEAD^{commit}",
        ])?)
        .map_err(invalid)?
        .trim()
        .to_owned();
        Ok(repo)
    }

    fn command(&self) -> Command {
        let mut cmd = Command::new("git");
        cmd.current_dir(self.temporary.path())
            .args([
                "-c",
                "gc.auto=0",
                "-c",
                "maintenance.auto=false",
                "-c",
                "credential.interactive=false",
                "-c",
                "core.hooksPath=hooks",
            ])
            .env("GIT_CONFIG_NOSYSTEM", "1")
            .env("GIT_CONFIG_GLOBAL", self.temporary.path().join("config"))
            .env("GIT_TERMINAL_PROMPT", "0")
            .env_remove("GIT_DIR")
            .env_remove("GIT_WORK_TREE")
            .env_remove("GIT_CONFIG_COUNT")
            .env_remove("GIT_CONFIG_PARAMETERS")
            .stdin(Stdio::null());
        if std::env::var_os("GIT_SSH_COMMAND").is_none() {
            cmd.env("GIT_SSH_COMMAND", "ssh -oBatchMode=yes");
        }
        cmd
    }

    fn run(&self, args: &[&str]) -> Result<Vec<u8>, Error> {
        let output = self
            .command()
            .args(args)
            .output()
            .map_err(|e| invalid(format!("cannot run Git: {e}")))?;
        if !output.status.success() {
            return Err(invalid(format!(
                "Git template resolution failed: {}",
                String::from_utf8_lossy(&output.stderr).trim()
            )));
        }
        Ok(output.stdout)
    }

    /// Export a repository subdirectory as ordinary files. Git object modes are checked before
    /// writing: symlinks and submodules are rejected, and nothing from the repo is executed.
    pub fn directory(&self, subdir: Option<&str>) -> Result<PathBuf, Error> {
        let tree = match subdir {
            Some(path) => {
                safe_path(path)?;
                format!("{}:{path}", self.commit)
            }
            None => self.commit.clone(),
        };
        let listing = self.run(&["--git-dir=objects", "ls-tree", "-r", "-z", &tree])?;
        let destination = self.temporary.path().join("export");
        if destination.exists() {
            return Err(invalid("repository snapshot already exported"));
        }
        std::fs::create_dir(&destination).map_err(invalid)?;
        let mut bytes = 0usize;
        let mut count = 0;
        for record in listing.split(|b| *b == 0).filter(|s| !s.is_empty()) {
            count += 1;
            if count > MAX_FILES {
                return Err(invalid(
                    "repository template directory exceeds 4096 files; select a narrower --template-path or --source",
                ));
            }
            let record = std::str::from_utf8(record).map_err(invalid)?;
            let (meta, path) = record
                .split_once('\t')
                .ok_or_else(|| invalid("invalid Git tree entry"))?;
            // Git metadata is never exported. Other dotfiles are invalid bundle assets.
            if path.split('/').any(|p| p.starts_with('.')) {
                continue;
            }
            safe_path(path)?;
            let parts: Vec<_> = meta.split_whitespace().collect();
            if parts.len() != 3 || parts[1] != "blob" || !matches!(parts[0], "100644" | "100755") {
                return Err(invalid(format!(
                    "repository template contains a symlink or submodule: {path}"
                )));
            }
            let size =
                String::from_utf8(self.run(&["--git-dir=objects", "cat-file", "-s", parts[2]])?)
                    .map_err(invalid)?
                    .trim()
                    .parse::<usize>()
                    .map_err(invalid)?;
            bytes = bytes
                .checked_add(size)
                .ok_or_else(|| invalid("template size overflow"))?;
            if bytes > MAX_BYTES {
                return Err(invalid(
                    "repository template directory exceeds 32 MiB; select a narrower source",
                ));
            }
            let content = self.run(&["--git-dir=objects", "cat-file", "blob", parts[2]])?;
            std::str::from_utf8(&content)
                .map_err(|_| invalid(format!("template asset must be UTF-8: {path}")))?;
            let target = destination.join(path);
            std::fs::create_dir_all(target.parent().expect("export file parent"))
                .map_err(invalid)?;
            std::fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(target)
                .map_err(invalid)?
                .write_all(&content)
                .map_err(invalid)?;
        }
        Ok(destination)
    }

    /// Replace a temporary local identity with reproducible repository provenance.
    pub fn identify(&self, template: &mut Template, subdir: Option<&str>) {
        let exported = self
            .temporary
            .path()
            .join("export")
            .canonicalize()
            .unwrap_or_else(|_| self.temporary.path().join("export"));
        let prefix = format!("local:{}", exported.display());
        let relative = template
            .source
            .strip_prefix(&prefix)
            .unwrap_or("")
            .trim_start_matches(['/', '\\'])
            .replace('\\', "/");
        let path = [subdir.unwrap_or(""), relative.as_str()]
            .into_iter()
            .filter(|s| !s.is_empty())
            .collect::<Vec<_>>()
            .join("/");
        template.source = format!(
            "git:{}#{}:{}",
            self.url,
            self.commit,
            if path.is_empty() { "." } else { &path }
        );
    }
}
