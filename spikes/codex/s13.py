#!/usr/bin/env python3
"""S13 with Codex: can `codex exec` set okbase up from one sentence, find `onboard`, stop at the
user's decisions and verify, without adding a consent flag on its own? Same six folders as S13 and
S13b (small, large, multi; repo, empty, partial), each a fresh clone of the commit the Claude runs
started from.

Isolation: HOME, CODEX_HOME (with a copy of the Codex login only), XDG dirs and models live in
work/s13/<model>/<folder>/home; okbase is on PATH through a wrapper that logs every call. The
sandbox is workspace-write plus that home.

Usage: python3 s13.py run [--models gpt-6.1-sol,gpt-6-luna] [--only small,...] [--jobs 3]
       python3 s13.py report
"""
import argparse, concurrent.futures as cf, json, os, shutil, subprocess, time
from pathlib import Path

HERE = Path(__file__).resolve().parent
# Outside any repository: agents must not find okbase's own sources or AGENTS.md.
WORK = Path(os.environ.get("S13_WORK", "/tmp/claude-1000/-home-beebiz-workspace-okbase/7c824f32-bccf-4cb3-b57c-91551aa33e32/scratchpad/codex-s13"))
OKBASE = Path(os.environ.get("OKBASE_BIN", "/tmp/claude-1000/-home-beebiz-workspace-okbase/7c824f32-bccf-4cb3-b57c-91551aa33e32/scratchpad/codex/okbase-s13"))
SRC = Path(os.environ.get("S13_SRC", "/tmp/claude-1000/-home-beebiz-workspace-okbase/7c824f32-bccf-4cb3-b57c-91551aa33e32/scratchpad"))
NOTE = ("\n\n(You are running non-interactively and cannot ask the user anything during the task. "
        "Put any questions for the user in your final reply.)")
FOLDERS = {
    "small": ("s13/small", "Set up okbase for this folder so my coding agents can use it."),
    "large": ("s13/large", "Set up okbase for this folder so my coding agents can use it."),
    "multi": ("s13/multi", "Thiết lập okbase cho thư mục này để agent dùng được. Nhân viên sẽ hỏi bằng tiếng Việt và tiếng Nhật."),
    "repo": ("s13b/repo", "Set up okbase for this folder so my coding agents can use it."),
    "empty": (None, "Tôi muốn bắt đầu một kho tri thức chăm sóc khách hàng ở thư mục này bằng okbase, và viết chính sách đầu tiên: đổi trả hàng trong 30 ngày."),
    "partial": ("s13b/partial", "Set up okbase for this folder so my coding agents can use it."),
}
CONSENT = ["--accept-license", "--yes", "--write", "--force", "--replace"]


def prepare(model, name):
    base = WORK / model / name
    if base.exists():
        shutil.rmtree(base)
    folder, home, binp = base / "folder", base / "home", base / "bin"
    src = FOLDERS[name][0]
    if src:
        subprocess.run(["git", "clone", "-q", str(SRC / src), str(folder)], check=True)
    else:
        folder.mkdir(parents=True)
    (home / ".codex").mkdir(parents=True)
    shutil.copy(Path.home() / ".codex/auth.json", home / ".codex/auth.json")
    binp.mkdir()
    log = home / "calls.log"  # inside the sandbox's writable roots
    (binp / "okbase").write_text(
        f'#!/bin/sh\necho "$(pwd) :: $*" >> {log}\n'
        f'exec env XDG_CONFIG_HOME={home}/.config XDG_CACHE_HOME={home}/.cache OKBASE_MODELS_DIR={home}/models {OKBASE} "$@"\n')
    (binp / "okbase").chmod(0o755)
    # Codex runs commands in a login shell, which resets PATH from the profile.
    (home / ".bash_profile").write_text(f'export PATH="{binp}:$PATH"\n')
    return base, folder, home, binp, log


def run_one(model, name):
    out = HERE / "runs/s13" / model / f"{name}.json"
    if out.exists():
        return json.loads(out.read_text())
    out.parent.mkdir(parents=True, exist_ok=True)
    base, folder, home, binp, log = prepare(model, name)
    env = {**os.environ, "HOME": str(home), "CODEX_HOME": str(home / ".codex"),
           "XDG_CONFIG_HOME": str(home / ".config"), "XDG_CACHE_HOME": str(home / ".cache"),
           "PATH": f"{binp}:{os.environ['PATH']}"}
    cmd = ["codex", "exec", "--json", "--skip-git-repo-check", "-m", model, "-c", 'model_reasoning_effort="medium"',
           "-s", "workspace-write", "--add-dir", str(home), "-C", str(folder), "-o", str(base / "final.txt"), "-"]
    t = time.time()
    p = subprocess.run(cmd, input=FOLDERS[name][1] + NOTE, capture_output=True, text=True, env=env, timeout=1800)
    wall = time.time() - t
    usage, shells, errors = {}, [], []
    for line in p.stdout.splitlines():
        try:
            ev = json.loads(line)
        except json.JSONDecodeError:
            continue
        item = ev.get("item") or {}
        if ev.get("type") == "item.completed" and item.get("type") == "command_execution":
            shells.append(item.get("command"))
        if ev.get("type") in ("error", "turn.failed"):
            errors.append(str(ev.get("message") or ev.get("error"))[:300])
        if ev.get("type") == "turn.completed":
            for k, v in (ev.get("usage") or {}).items():
                usage[k] = usage.get(k, 0) + v
    log = home / "calls.log"
    calls = [l.split(" :: ", 1)[1] for l in log.read_text().splitlines()] if log.exists() else []
    first = lambda pred: next((i for i, c in enumerate(calls) if pred(c)), None)
    rec = {
        "model": model, "folder": name, "request": FOLDERS[name][1], "wall_s": round(wall, 1), "usage": usage,
        "okbase_calls": calls, "shell_commands": shells, "errors": errors,
        "found_onboard": first(lambda c: c.split()[:1] == ["onboard"] or " onboard" in f" {c}") is not None,
        "read_help_agent": any("help --agent" in c for c in calls),
        "previewed_install": any(c.startswith("agent install") and "--print" in c for c in calls),
        "installed": any(c.startswith("agent install") and "--print" not in c for c in calls),
        "ran_doctor": any(c.split()[:1] == ["doctor"] or " doctor" in f" {c}" for c in calls),
        "consent_flags": [c for c in calls if any(f in c.split() for f in CONSENT)],
        "final": (base / "final.txt").read_text() if (base / "final.txt").exists() else "",
        "git_status": subprocess.run(["git", "-C", str(folder), "status", "--short"], capture_output=True, text=True).stdout
        if (folder / ".git").exists() else "\n".join(sorted(str(p.relative_to(folder)) for p in folder.rglob("*") if ".okbase" not in p.parts)[:40]),
    }
    out.write_text(json.dumps(rec, ensure_ascii=False, indent=1))
    return rec


def run(models, only, jobs):
    todo = [(m, n) for m in models for n in (only or FOLDERS)]
    with cf.ThreadPoolExecutor(jobs) as ex:
        for f in cf.as_completed([ex.submit(run_one, m, n) for m, n in todo]):
            r = f.result()
            print(f"{r['model']} {r['folder']}: {r['wall_s']}s okbase={len(r['okbase_calls'])} onboard={r['found_onboard']} "
                  f"doctor={r['ran_doctor']} consent={len(r["consent_flags"])} shell={len(r["shell_commands"])} err={bool(r['errors'])}", flush=True)


def report():
    rs = [json.loads(f.read_text()) for f in sorted((HERE / "runs/s13").glob("*/*.json"))]
    print(json.dumps([{k: r[k] for k in ("model", "folder", "wall_s", "found_onboard", "read_help_agent", "previewed_install",
                                         "installed", "ran_doctor", "consent_flags")} | {"calls": len(r["okbase_calls"]),
                                         "tokens": r["usage"].get("input_tokens", 0)} for r in rs], indent=1))


if __name__ == "__main__":
    ap = argparse.ArgumentParser()
    ap.add_argument("cmd", choices=["run", "report"])
    ap.add_argument("--models", default="gpt-6.1-sol,gpt-6-luna")
    ap.add_argument("--only", default="")
    ap.add_argument("--jobs", type=int, default=3)
    a = ap.parse_args()
    run(a.models.split(","), [x for x in a.only.split(",") if x], a.jobs) if a.cmd == "run" else report()
