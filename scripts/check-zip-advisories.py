#!/usr/bin/env python3
"""Fail closed when the reviewed ZIP advisory exceptions need reassessment."""
import hashlib
import json
import subprocess
import sys
import tomllib
from datetime import date
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
POLICY = ROOT / 'docs/security/zip-advisories.json'


def source_digest(root):
    digest = hashlib.sha256()
    for path in sorted((root / 'src').rglob('*.rs')):
        digest.update(path.relative_to(root).as_posix().encode() + b'\0')
        digest.update(path.read_bytes() + b'\0')
    return digest.hexdigest()


def check(data, policy, today):
    if today >= date.fromisoformat(policy['review_before']):
        raise ValueError('ZIP advisory assessment expired; review upstream fixes and applicability')
    packages = {p['id']: p for p in data['packages']}
    zip_ids = {p['id'] for p in data['packages'] if p['name'] == 'zip'}
    if not zip_ids:
        raise ValueError('zip was removed; remove the obsolete advisory exceptions')
    actual = {packages[i]['version'] for i in zip_ids}
    if actual != {policy['zip_version']}:
        raise ValueError(f'ZIP versions changed: {sorted(actual)}; reassess/remove exceptions')
    parents = {node['id'] for node in data['resolve']['nodes']
               if any(dep['pkg'] in zip_ids for dep in node['deps'])}
    expected = {(name, item['version']) for name, item in policy['readers'].items()}
    actual = {(packages[i]['name'], packages[i]['version']) for i in parents}
    if actual != expected:
        raise ValueError(f'ZIP dependency parents changed: {sorted(actual)}; reassess exceptions')
    for package_id in parents:
        package = packages[package_id]
        reviewed = policy['readers'][package['name']]
        if package['source'] != 'registry+https://github.com/rust-lang/crates.io-index':
            raise ValueError(f'{package["name"]} is no longer a crates.io package')
        root = Path(package['manifest_path']).parent
        if source_digest(root) != reviewed['source_sha256']:
            raise ValueError(f'{package["name"]} source differs from the reviewed source')


def main():
    policy = json.loads(POLICY.read_text())
    config = tomllib.loads((ROOT / 'deny.toml').read_text())
    ignored = {item['id'] for item in config['advisories']['ignore']}
    if not set(policy['advisories']) <= ignored:
        sys.exit('ZIP policy and cargo-deny exceptions disagree; remove or reassess them together')
    result = subprocess.run(['cargo', 'metadata', '--all-features', '--locked', '--format-version', '1'],
                            cwd=ROOT, check=True, text=True, capture_output=True)
    try:
        check(json.loads(result.stdout), policy, date.today())
    except ValueError as error:
        sys.exit(str(error))
    print('ZIP advisory assessment matches reviewed readers, sources and graph; review before '
          + policy['review_before'])


if __name__ == '__main__':
    main()
