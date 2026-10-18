#!/usr/bin/env python3
"""Validate an immutable release tag and report applicable workflow outputs."""
import importlib.util
import os
import re
import subprocess
import sys
import tomllib
from pathlib import Path

root = Path(__file__).resolve().parent.parent
os.chdir(root)
tag = sys.argv[1]
if not re.fullmatch(r"v(0|[1-9]\d*)\.(0|[1-9]\d*)\.(0|[1-9]\d*)(-rc\.(0|[1-9]\d*))?", tag):
    sys.exit(f"invalid release tag: {tag}")
subprocess.run(["git", "merge-base", "--is-ancestor", f"{tag}^{{commit}}", "origin/main"], check=True)
if subprocess.check_output(["git", "rev-parse", "HEAD"]) != subprocess.check_output(["git", "rev-parse", f"{tag}^{{commit}}"]):
    sys.exit("checkout does not match release tag")
version = tomllib.loads((root / "Cargo.toml").read_text())["workspace"]["package"]["version"]
if tag != f"v{version}":
    sys.exit(f"tag {tag} does not match workspace {version}")
if not re.search(rf"^## \[{re.escape(version)}\](?:\s|$)", (root / "CHANGELOG.md").read_text(), re.M):
    sys.exit(f"missing changelog entry for {version}")
spec = importlib.util.spec_from_file_location("release", root / "scripts/release.py")
release = importlib.util.module_from_spec(spec)
spec.loader.exec_module(release)
release.publish_order()
data = release.metadata()
binaries = any("bin" in t["kind"] for p in data["packages"] for t in p["targets"])
outputs = {"tag": tag, "version": version, "prerelease": str("-" in version).lower(),
           "binaries": str(binaries).lower(), "image": str((root / "Dockerfile").is_file()).lower()}
for key, value in outputs.items():
    print(f"{key}={value}")
if os.environ.get("GITHUB_OUTPUT"):
    with open(os.environ["GITHUB_OUTPUT"], "a") as out:
        out.writelines(f"{key}={value}\n" for key, value in outputs.items())
