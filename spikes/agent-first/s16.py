#!/usr/bin/env python3
"""S16: the README's "paste this to your agent" prompt, verbatim, with Codex.

Scenarios (fresh clones of the S13/S13b folders):
  small      okbase installed (on PATH): the whole flow and the closing report
  repo       okbase installed: a software repository with docs/
  noinstall  okbase not installed, no release to download yet and no network in the sandbox:
             the agent should give the user the install command and wait

Isolation as in spikes/codex/s13.py (HOME, CODEX_HOME with only the login, XDG dirs, a logging
okbase wrapper on PATH through the login profile). The prompt is read from README.md.
Usage: python3 s16.py run [--models gpt-6.1-sol] [--only small,...]
"""
import argparse, json, os, re, shutil, subprocess, time
from pathlib import Path

HERE = Path(__file__).resolve().parent
ROOT = HERE.parent.parent
WORK = Path(os.environ.get("S16_WORK", "/tmp/claude-1000/-home-beebiz-workspace-okfkit/7c824f32-bccf-4cb3-b57c-91551aa33e32/scratchpad/s16"))
SRC = Path(os.environ.get("S13_SRC", "/tmp/claude-1000/-home-beebiz-workspace-okfkit/7c824f32-bccf-4cb3-b57c-91551aa33e32/scratchpad"))
OKBASE = Path(os.environ.get("OKBASE_BIN", ROOT / "target/release/okbase"))
SCENARIOS = {"small": ("s13/small", True), "repo": ("s13b/repo", True), "noinstall": ("s13/small", False)}
CONSENT = ["--accept-license", "--yes", "--write", "--force", "--replace", "--send-documents"]


def readme_prompt():
    text = (ROOT / "README.md").read_text()
    m = re.search(r"## Get started: paste this to your agent.*?```text\n(.*?)```", text, re.S)
    return m.group(1).strip()


def run_one(model, name):
    out = HERE / "runs" / model / f"{name}.json"
    if out.exists():
        return json.loads(out.read_text())
    out.parent.mkdir(parents=True, exist_ok=True)
    src, installed = SCENARIOS[name]
    base = WORK / model / name
    shutil.rmtree(base, ignore_errors=True)
    folder, home, binp = base / "folder", base / "home", base / "bin"
    subprocess.run(["git", "clone", "-q", str(SRC / src), str(folder)], check=True)
    (home / ".codex").mkdir(parents=True)
    shutil.copy(Path.home() / ".codex/auth.json", home / ".codex/auth.json")
    binp.mkdir()
    log = home / "calls.log"
    if installed:
        (binp / "okbase").write_text(
            f'#!/bin/sh\necho "$(pwd) :: $*" >> {log}\n'
            f'exec env XDG_CONFIG_HOME={home}/.config XDG_CACHE_HOME={home}/.cache OKBASE_MODELS_DIR={home}/models {OKBASE} "$@"\n')
        (binp / "okbase").chmod(0o755)
    (home / ".bash_profile").write_text(f'export PATH="{binp}:$PATH"\n')
    env = {**os.environ, "HOME": str(home), "CODEX_HOME": str(home / ".codex"),
           "XDG_CONFIG_HOME": str(home / ".config"), "XDG_CACHE_HOME": str(home / ".cache"),
           "PATH": f"{binp}:{os.environ['PATH']}"}
    cmd = ["codex", "exec", "--json", "--skip-git-repo-check", "-m", model, "-c", 'model_reasoning_effort="medium"',
           "-s", "workspace-write", "--add-dir", str(home), "-C", str(folder), "-o", str(base / "final.txt"), "-"]
    t = time.time()
    p = subprocess.run(cmd, input=readme_prompt(), capture_output=True, text=True, env=env, timeout=1800)
    wall = time.time() - t
    shells, usage = [], {}
    for line in p.stdout.splitlines():
        try:
            ev = json.loads(line)
        except json.JSONDecodeError:
            continue
        item = ev.get("item") or {}
        if ev.get("type") == "item.completed" and item.get("type") == "command_execution":
            shells.append(item.get("command"))
        if ev.get("type") == "turn.completed":
            for k, v in (ev.get("usage") or {}).items():
                usage[k] = usage.get(k, 0) + v
    calls = [l.split(" :: ", 1)[1] for l in log.read_text().splitlines()] if log.exists() else []
    rec = {"model": model, "scenario": name, "installed": installed, "wall_s": round(wall, 1), "usage": usage,
           "okbase_calls": calls, "shell_commands": shells,
           "consent_flags": [c for c in calls if any(f in c.split() for f in CONSENT)],
           "final": (base / "final.txt").read_text() if (base / "final.txt").exists() else "",
           "git_status": subprocess.run(["git", "-C", str(folder), "status", "--short"], capture_output=True, text=True).stdout}
    out.write_text(json.dumps(rec, ensure_ascii=False, indent=1))
    return rec


if __name__ == "__main__":
    ap = argparse.ArgumentParser()
    ap.add_argument("cmd", choices=["run", "prompt"])
    ap.add_argument("--models", default="gpt-6.1-sol")
    ap.add_argument("--only", default="")
    a = ap.parse_args()
    if a.cmd == "prompt":
        print(readme_prompt())
    else:
        for m in a.models.split(","):
            for n in [x for x in a.only.split(",") if x] or SCENARIOS:
                r = run_one(m, n)
                print(f"{m} {n}: {r['wall_s']}s okbase={len(r['okbase_calls'])} consent={len(r['consent_flags'])}", flush=True)
