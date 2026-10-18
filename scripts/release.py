#!/usr/bin/env python3
"""Release helpers for okbase (the procedure is in docs/releasing.md).

    scripts/release.py version 0.5.0       set the workspace version and every internal dependency
    scripts/release.py order               print the crates in publishing order
    scripts/release.py pending             print the crates whose version is not on crates.io yet
    scripts/release.py publish [--dry-run [--allow-dirty]]
                                           publish every crate to crates.io, in order
    scripts/release.py trustpub OWNER/REPO [--workflow F] [--environment E] [--crates a,b] [--only]
                                           set up crates.io Trusted Publishing for the crates

`publish` can be run again after a failure: versions already on crates.io are skipped. When
crates.io rate-limits new crates (HTTP 429, about one new crate every 10 minutes after a short
burst), it waits until the time crates.io gives and retries.

`trustpub` needs a crates.io API token with the `trusted-publishing` scope: the token saved by `cargo login`.
It lets the GitHub workflow publish the crates (by default this workspace's) without any stored
token; `--only` also makes crates.io refuse every other way of publishing them. It works for any
repository: `--crates` names the crates, `--workflow` and `--environment` default to
`release.yml` and `crates-io`. Running it again changes nothing.
"""

import email.utils
import itertools
import json
import os
import re
import subprocess
import sys
import time
import tomllib
import urllib.error
import urllib.request
from datetime import datetime, timezone
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
SEMVER = re.compile(r"^(0|[1-9]\d*)\.(0|[1-9]\d*)\.(0|[1-9]\d*)(-rc\.(0|[1-9]\d*))?$")


def metadata():
    out = subprocess.run(
        ["cargo", "metadata", "--format-version", "1", "--no-deps"],
        cwd=ROOT, check=True, capture_output=True, text=True,
    ).stdout
    return json.loads(out)


def publish_order():
    """Publishable workspace crates, dependencies (dev-dependencies too) before dependents."""
    data = metadata()
    members = set(data["workspace_members"])
    packages = {p["name"]: p for p in data["packages"]
                if p["id"] in members and p.get("publish") != []
                and (p.get("publish") is None or "crates-io" in p["publish"])}
    deps = {
        name: {d["name"] for d in p["dependencies"] if d.get("path") and d["name"] in packages}
        for name, p in packages.items()
    }
    order, done = [], set()
    while len(order) < len(packages):
        ready = sorted(n for n in packages if n not in done and deps[n] <= done)
        if not ready:
            sys.exit(f"dependency cycle among {sorted(set(packages) - done)}")
        order.extend(ready)
        done.update(ready)
    version = {p["version"] for p in data["packages"] if p["id"] in members}
    if len(version) != 1:
        sys.exit(f"crates have different versions: {sorted(version)}")
    return order, version.pop()


def set_version(version):
    if not SEMVER.match(version):
        sys.exit(f"`{version}` is not a version like 0.5.0 or 0.6.0-rc.1")
    data = metadata()
    paths = [ROOT / "Cargo.toml"] + [Path(p["manifest_path"]) for p in data["packages"]]
    for path in dict.fromkeys(paths):
        text = path.read_text()
        text = re.sub(r'(\[(?:workspace\.package|package)\][^\[]*?\nversion\s*=\s*)"[^"]*"',
                      lambda m: m[1] + f'"{version}"', text, count=1)
        # Inline path dependencies, including target-specific and dev dependencies.
        def dependency(m):
            entry = m[0]
            if re.search(r'\bpath\s*=', entry):
                entry = re.sub(r'(\bversion\s*=\s*)"[^"]*"',
                               lambda v: v[1] + f'"{version}"', entry)
            return entry
        text = re.sub(r'(?m)^[-\w]+\s*=\s*\{[^}]*\}', dependency, text)
        # Explicit dependency tables.
        text = re.sub(r'(\[(?:[^]\n]*\.)?(?:dependencies|dev-dependencies|build-dependencies)\.[^]\n]+\])([^\[]*)',
                      lambda m: m[1] + (re.sub(r'(?m)^(version\s*=\s*)"[^"]*"',
                        lambda v: v[1] + f'"{version}"', m[2]) if re.search(r'(?m)^path\s*=', m[2]) else m[2]), text)
        path.write_text(text)
    subprocess.run(["cargo", "update", "--workspace", "--quiet"], cwd=ROOT, check=True)
    publish_order()
    print(f"version {version}")


def index_versions(name, check_equivalents=True):
    """Versions of `name` on crates.io (from the sparse index), or None for a crate it does not
    have yet."""
    lower = name.lower()
    if len(lower) <= 2:
        prefix = str(len(lower))
    elif len(lower) == 3:
        prefix = f"3/{lower[0]}"
    else:
        prefix = f"{lower[:2]}/{lower[2:4]}"
    url = f"https://index.crates.io/{prefix}/{lower}"
    try:
        with request_with_retry(urllib.request.Request(url, headers={"User-Agent": USER_AGENT})) as response:
            lines = response.read().decode().splitlines()
    except urllib.error.HTTPError as error:
        if error.code == 404:
            if check_equivalents:
                positions = [i for i, char in enumerate(name) if char in "-_"]
                for choices in itertools.product("-_", repeat=len(positions)):
                    spelling = list(name)
                    for i, char in zip(positions, choices):
                        spelling[i] = char
                    alias = "".join(spelling)
                    if alias != name and index_versions(alias, False) is not None:
                        sys.exit(f"crate name {name} collides with existing equivalent {alias}; preserve the registered name")
            return None
        raise
    return {row["vers"]: row for line in lines if line.strip() for row in [json.loads(line)]}


def published(name, version):
    """Whether crates.io already has `name` at `version`."""
    versions = index_versions(name) or {}
    if versions.get(version, {}).get("yanked"):
        sys.exit(f"{name} {version} is yanked; choose a new release version")
    return version in versions


def pending():
    """Prints the crates still to publish. With GITHUB_OUTPUT set, also writes `count` and `new`
    (crates crates.io does not have at all: Trusted Publishing cannot create them)."""
    order, version = publish_order()
    todo, new = [], []
    for name in order:
        versions = index_versions(name)
        if versions is None:
            new.append(name)
        if versions and versions.get(version, {}).get("yanked"):
            sys.exit(f"{name} {version} is yanked; choose a new release version")
        if version not in (versions or {}):
            todo.append(name)
    for name in todo:
        print(f"{name} {version}" + (" (new crate)" if name in new else ""))
    if not todo:
        print(f"every crate is on crates.io at {version}")
    github_output = os.environ.get("GITHUB_OUTPUT")
    if github_output:
        with open(github_output, "a") as out:
            out.write(f"count={len(todo)}\nnew={' '.join(new)}\n")


USER_AGENT = "okbase-release (https://github.com/tidusvn05/okbase)"


def crates_io(method, path, token, body=None):
    request = urllib.request.Request(
        f"https://crates.io/api/v1{path}",
        method=method,
        data=None if body is None else json.dumps(body).encode(),
        headers={
            "Authorization": token,
            "User-Agent": USER_AGENT,
            "Content-Type": "application/json",
            "Accept": "application/json",
        },
    )
    try:
        with request_with_retry(request) as response:
            return json.loads(response.read() or b"{}")
    except urllib.error.HTTPError as error:
        sys.exit(f"crates.io {method} {path}: HTTP {error.code}")


def cargo_token():
    """The crates.io token saved by `cargo login`, if any."""
    home = Path(os.environ.get("CARGO_HOME", Path.home() / ".cargo"))
    for name in ("credentials.toml", "credentials"):
        path = home / name
        if path.is_file():
            with open(path, "rb") as f:
                return tomllib.load(f).get("registry", {}).get("token", "")
    return ""


def trustpub(args):
    token = cargo_token()
    if not token:
        sys.exit("run `cargo login` with a crates.io token that has the "
                 "`trusted-publishing` scope")
    if not args or "/" not in args[0]:
        sys.exit(__doc__)
    owner, repo = args[0].split("/", 1)
    options = {"--workflow": "release.yml", "--environment": "crates-io", "--crates": None}
    only = False
    rest = args[1:]
    while rest:
        flag = rest.pop(0)
        if flag == "--only":
            only = True
        elif flag in options and rest:
            options[flag] = rest.pop(0)
        else:
            sys.exit(__doc__)
    crates = (options["--crates"].split(",") if options["--crates"] else publish_order()[0])
    environment = options["--environment"] or None
    for name in crates:
        if index_versions(name) is None:
            sys.exit(f"{name} does not exist; approved manual bootstrap is required first")
        owners = crates_io("GET", f"/crates/{name}/owners", token)
        print(f"{name} owners: " + ", ".join(user["login"] for user in owners["users"]))
        existing = crates_io("GET", f"/trusted_publishing/github_configs?crate={name}", token)
        same = [
            c for c in existing.get("github_configs", [])
            if c["repository_owner"].lower() == owner.lower()
            and c["repository_name"].lower() == repo.lower()
            and c["workflow_filename"] == options["--workflow"]
            and c.get("environment") == environment
        ]
        if same:
            print(f"= {name}: already trusts {owner}/{repo} {options['--workflow']}")
        else:
            crates_io("POST", "/trusted_publishing/github_configs", token, {
                "github_config": {
                    "crate": name,
                    "repository_owner": owner,
                    "repository_name": repo,
                    "workflow_filename": options["--workflow"],
                    "environment": environment,
                }
            })
            print(f"+ {name}: trusts {owner}/{repo} {options['--workflow']}"
                  + (f" (environment {environment})" if environment else ""))
        if only:
            info = crates_io("GET", f"/crates/{name}", token)
            if not info["crate"].get("trustpub_only"):
                crates_io("PATCH", f"/crates/{name}", token, {"crate": {"trustpub_only": True}})
            print(f"  {name}: Trusted Publishing only")


def retry_after(output):
    """Seconds to wait from a crates.io 429 message ("Please try again after <date>")."""
    match = re.search(r"try again after ([^.]+?(?:GMT|UTC|\+0000))", output)
    if match:
        try:
            when = email.utils.parsedate_to_datetime(match.group(1))
            return max(30, int((when - datetime.now(timezone.utc)).total_seconds()) + 15)
        except (TypeError, ValueError):
            pass
    return 600


def publish(dry_run, allow_dirty=False):
    order, version = publish_order()
    if dry_run:
        command = ["cargo", "publish", "--workspace", "--dry-run", "--locked", "--registry", "crates-io"]
        for p in metadata()["packages"]:
            if p["name"] not in order:
                command += ["--exclude", p["name"]]
        if allow_dirty:
            command += ["--allow-dirty"]
        subprocess.run(command, cwd=ROOT, check=True)
        print(f"packaged {len(order)} crates at {version}")
        return
    if "-" in version:
        sys.exit(f"refusing to publish the pre-release version {version}")
    if subprocess.check_output(["git", "status", "--porcelain"], cwd=ROOT).strip():
        sys.exit("publishing requires a clean working tree")
    # Preflight every crate before any upload, including yanks and new crates.
    missing = [name for name in order if index_versions(name) is None]
    if missing:
        sys.exit("new crates require explicitly approved manual bootstrap: " + ", ".join(missing))
    todo = [name for name in order if not published(name, version)]
    for name in todo:
        command = ["cargo", "publish", "--locked", "--registry", "crates-io", "-p", name]
        for attempt in range(20):
            if published(name, version):
                break
            print(f"> {' '.join(command)}", flush=True)
            run = subprocess.run(command, cwd=ROOT, capture_output=True, text=True)
            output = run.stdout + run.stderr
            if run.returncode == 0:
                break
            if "429" in output or "Too Many Requests" in output:
                wait = retry_after(output)
                print(f"rate-limited; retrying in {wait}s", flush=True)
                time.sleep(wait)
                continue
            if "already exists" in output and published(name, version):
                break
            sys.stderr.write(output)
            sys.exit(f"publishing {name} failed")
        else:
            sys.exit(f"publishing {name}: retry limit reached; rerun safely")
        for attempt in range(30):
            if published(name, version):
                break
            time.sleep(10)
        else:
            sys.exit(f"{name} {version} not visible in index yet; retry later")
    print(f"all {len(order)} crates are published at {version}")


def request_with_retry(request):
    """Honor Retry-After (seconds or HTTP date) on registry rate limits."""
    for attempt in range(20):
        try:
            return urllib.request.urlopen(request, timeout=30)
        except urllib.error.HTTPError as error:
            if error.code != 429 or attempt == 19:
                raise
            value = error.headers.get("Retry-After", "600")
            try:
                delay = float(value)
            except ValueError:
                delay = (email.utils.parsedate_to_datetime(value) - datetime.now(timezone.utc)).total_seconds()
            print(f"registry rate limit; retrying in {max(1, delay):.0f}s", flush=True)
            time.sleep(max(1, delay))


def main():
    args = sys.argv[1:]
    if args[:1] == ["version"] and len(args) == 2:
        set_version(args[1])
    elif args == ["order"]:
        order, version = publish_order()
        print(f"{version}: " + " ".join(order))
    elif args == ["pending"]:
        pending()
    elif args[:1] == ["publish"] and set(args[1:]) <= {"--dry-run", "--allow-dirty"}:
        if "--allow-dirty" in args and "--dry-run" not in args:
            sys.exit("--allow-dirty is only permitted with --dry-run")
        publish("--dry-run" in args, "--allow-dirty" in args)
    elif args[:1] == ["trustpub"]:
        trustpub(args[1:])
    else:
        sys.exit(__doc__)


if __name__ == "__main__":
    main()
