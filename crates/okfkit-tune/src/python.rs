//! The private Python environment that trains and exports tuned models (feature `train`).
//!
//! okfkit has no Python dependency until the user runs `okfkit embed tune train/export` and
//! agrees to the download. The environment lives in `<user cache>/okfkit/tune-env/<hash>/`, with
//! the exact package versions of spike S11 (`python/requirements.txt`) and the training script
//! embedded in this crate (`python/okfkit_tune.py`). `uv` is used when installed, else
//! `python3 -m venv` and pip.

use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use crate::Error;

/// The training and export script.
pub const SCRIPT: &str = include_str!("../python/okfkit_tune.py");
/// Pinned packages (torch is added separately: CPU wheels unless a CUDA GPU is present).
pub const REQUIREMENTS: &str = include_str!("../python/requirements.txt");
/// torch version measured in spike S11.
pub const TORCH: &str = "torch==2.14.1";
/// Unsloth version for CUDA machines.
pub const UNSLOTH: &str = "unsloth==2026.9.14";
/// CPU-only torch wheels (much smaller than the CUDA build).
pub const TORCH_CPU_INDEX: &str = "https://download.pytorch.org/whl/cpu";
/// Approximate download, for the consent prompt.
pub const DOWNLOAD_HINT: &str = "about 1 GB (CPU) or 3 GB (CUDA GPU)";

/// A ready environment.
#[derive(Debug, Clone)]
pub struct PyEnv {
    /// Environment directory.
    pub dir: PathBuf,
    /// Its Python interpreter.
    pub python: PathBuf,
    /// Whether it was set up for a CUDA GPU.
    pub gpu: bool,
}

fn fail(what: impl Into<String>) -> Error {
    Error::Invalid(what.into())
}

/// Whether an NVIDIA GPU is usable (`nvidia-smi -L` lists one).
pub fn has_cuda() -> bool {
    Command::new("nvidia-smi")
        .arg("-L")
        .output()
        .is_ok_and(|o| o.status.success() && String::from_utf8_lossy(&o.stdout).contains("GPU"))
}

fn on_path(program: &str) -> bool {
    Command::new(program)
        .arg("--version")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .is_ok_and(|s| s.success())
}

/// The directory for this okfkit version's environment.
pub fn env_dir(cache_root: &Path, gpu: bool) -> PathBuf {
    let id = blake3::hash(format!("{REQUIREMENTS}{TORCH}{UNSLOTH}{gpu}").as_bytes()).to_hex();
    cache_root.join("okfkit").join("tune-env").join(format!(
        "{}-{}",
        if gpu { "cuda" } else { "cpu" },
        &id[..12]
    ))
}

fn venv_python(dir: &Path) -> PathBuf {
    if cfg!(windows) {
        dir.join("Scripts").join("python.exe")
    } else {
        dir.join("bin").join("python")
    }
}

/// The environment, if it is already set up.
pub fn existing(cache_root: &Path, gpu: bool) -> Option<PyEnv> {
    let dir = env_dir(cache_root, gpu);
    dir.join(".ready").is_file().then(|| PyEnv {
        python: venv_python(&dir),
        dir,
        gpu,
    })
}

/// Runs a command, sending each output line to `log`.
pub fn run_logged(cmd: &mut Command, log: &mut dyn FnMut(&str)) -> Result<(), Error> {
    let shown = format!("{cmd:?}");
    let mut child = cmd
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| fail(format!("cannot run {shown}: {e}")))?;
    let stderr = child.stderr.take();
    let err_thread = std::thread::spawn(move || {
        let mut lines = Vec::new();
        if let Some(s) = stderr {
            for line in BufReader::new(s).lines().map_while(Result::ok) {
                lines.push(line);
            }
        }
        lines
    });
    if let Some(out) = child.stdout.take() {
        for line in BufReader::new(out).lines().map_while(Result::ok) {
            log(&line);
        }
    }
    let status = child.wait().map_err(|e| fail(e.to_string()))?;
    let err_lines = err_thread.join().unwrap_or_default();
    if status.success() {
        return Ok(());
    }
    let tail: Vec<&String> = err_lines.iter().rev().take(15).collect();
    let tail: Vec<&str> = tail.into_iter().rev().map(String::as_str).collect();
    Err(fail(format!(
        "{shown} failed ({status}):\n{}",
        tail.join("\n")
    )))
}

/// Creates the environment (downloads packages). Call only after the user agreed.
pub fn create(cache_root: &Path, gpu: bool, log: &mut dyn FnMut(&str)) -> Result<PyEnv, Error> {
    let dir = env_dir(cache_root, gpu);
    if let Some(e) = existing(cache_root, gpu) {
        return Ok(e);
    }
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(dir.parent().unwrap_or(&dir)).map_err(|e| fail(e.to_string()))?;
    let python = std::env::var("OKFKIT_PYTHON").unwrap_or_else(|_| "python3".into());
    let uv = on_path("uv");
    log(&format!(
        "creating the training environment in {} ({DOWNLOAD_HINT}) with {}",
        dir.display(),
        if uv { "uv" } else { "venv + pip" }
    ));
    if uv {
        run_logged(
            Command::new("uv")
                .args(["venv", "--python", &python])
                .arg(&dir),
            log,
        )?;
    } else {
        run_logged(Command::new(&python).args(["-m", "venv"]).arg(&dir), log)?;
    }
    let py = venv_python(&dir);
    let req = dir.join("requirements.txt");
    std::fs::write(&req, REQUIREMENTS).map_err(|e| fail(e.to_string()))?;
    let pip = |args: &[&str]| -> Command {
        let mut c = if uv {
            let mut c = Command::new("uv");
            c.args(["pip", "install", "--python"]).arg(&py);
            c
        } else {
            let mut c = Command::new(&py);
            c.args(["-m", "pip", "install", "--disable-pip-version-check"]);
            c
        };
        c.args(args);
        c
    };
    if gpu {
        run_logged(&mut pip(&[TORCH, UNSLOTH]), log)?;
    } else {
        run_logged(&mut pip(&[TORCH, "--index-url", TORCH_CPU_INDEX]), log)?;
    }
    let req_s = req.to_string_lossy().into_owned();
    run_logged(&mut pip(&["-r", &req_s]), log)?;
    let env = PyEnv {
        dir: dir.clone(),
        python: py,
        gpu,
    };
    run_script(&env, &["check"], log)?;
    std::fs::write(dir.join(".ready"), "ok\n").map_err(|e| fail(e.to_string()))?;
    Ok(env)
}

/// Runs the embedded script with `args` (`train …`, `export …`, `check`).
pub fn run_script(env: &PyEnv, args: &[&str], log: &mut dyn FnMut(&str)) -> Result<(), Error> {
    let script = env.dir.join("okfkit_tune.py");
    std::fs::write(&script, SCRIPT).map_err(|e| fail(e.to_string()))?;
    run_logged(
        Command::new(&env.python)
            .arg(&script)
            .args(args)
            .env("PYTHONUNBUFFERED", "1")
            .env("TOKENIZERS_PARALLELISM", "false"),
        log,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn env_dir_depends_on_device_and_pins() {
        let root = Path::new("/c");
        assert_ne!(env_dir(root, true), env_dir(root, false));
        assert!(
            env_dir(root, false)
                .to_string_lossy()
                .starts_with("/c/okfkit/tune-env/cpu-")
        );
        assert!(REQUIREMENTS.contains("sentence-transformers==6.1.0"));
        assert!(SCRIPT.contains("def export("));
    }

    #[test]
    fn run_logged_reports_failures() {
        let mut lines = Vec::new();
        let ok = run_logged(
            Command::new("sh").args(["-c", "echo one; echo two"]),
            &mut |l| lines.push(l.to_owned()),
        );
        assert!(ok.is_ok() && lines == ["one", "two"]);
        let err = run_logged(
            Command::new("sh").args(["-c", "echo boom >&2; exit 3"]),
            &mut |_| {},
        )
        .unwrap_err();
        assert!(err.to_string().contains("boom"), "{err}");
    }
}
