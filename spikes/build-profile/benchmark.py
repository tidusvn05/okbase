#!/usr/bin/env python3
"""Compare clean test builds before changing debug-info defaults. Requires empty output paths."""
import argparse
import json
import os
from pathlib import Path
import platform
import subprocess
import time


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--output', type=Path, required=True)
    parser.add_argument('--workspace', type=Path, default=Path(__file__).resolve().parents[2])
    parser.add_argument('--jobs', type=int, default=4)
    args = parser.parse_args()
    root = args.workspace.resolve()
    if '[profile.dev' in (root / 'Cargo.toml').read_text():
        raise SystemExit('baseline checkout must not have dev profile overrides; use --workspace with the recorded base commit')
    output = args.output.resolve()
    output.mkdir(parents=True, exist_ok=True)
    env = {k: v for k, v in os.environ.items()
           if not k.startswith('CARGO_PROFILE_') and k not in ('CARGO_TARGET_DIR', 'RUSTFLAGS', 'CARGO_ENCODED_RUSTFLAGS')}
    env['CARGO_INCREMENTAL'] = '1'
    result = {'commit': subprocess.check_output(['git', 'rev-parse', 'HEAD'], cwd=root, text=True).strip(),
              'cargo': subprocess.check_output(['cargo', '--version'], text=True).strip(),
              'rustc': subprocess.check_output(['rustc', '--version'], text=True).strip(),
              'platform': platform.system() + ' ' + platform.machine(),
              'logical_cpus': os.cpu_count(), 'jobs': args.jobs, 'runs': []}
    # Baseline uses actual Cargo defaults; candidate changes only debug-info settings.
    cases = [
        ('baseline', []),
        ('line-tables', ['--config', 'profile.dev.debug="line-tables-only"', '--config', 'profile.dev.package."*".debug=false']),
    ]
    for name, settings in cases:
        target = output / name
        if target.exists():
            raise SystemExit(f'refusing to reuse a nonempty target directory: {target}')
        command = ['cargo', *settings, 'test', '--workspace', '--no-run', '--locked',
                   '--jobs', str(args.jobs), '--target-dir', str(target)]
        print(f'{name}: clean cargo test --workspace --no-run', flush=True)
        started = time.monotonic()
        with (output / f'{name}.log').open('w') as log:
            subprocess.run(command, cwd=root, env=env, stdout=log, stderr=subprocess.STDOUT, check=True)
        seconds = time.monotonic() - started
        debug = target / 'debug'
        disk_bytes = int(subprocess.check_output(['du', '-sk', str(debug)], text=True).split()[0]) * 1024
        apparent_bytes = sum(p.stat().st_size for p in debug.rglob('*') if p.is_file() and not p.is_symlink())
        binaries = sorted((p for p in (debug / 'deps').iterdir()
                           if p.is_file() and os.access(p, os.X_OK) and not p.name.startswith('lib')),
                          key=lambda p: p.stat().st_size, reverse=True)
        row = {'name': name, 'command': command[:-1] + [f'<output>/{name}'],
               'wall_seconds': round(seconds, 3), 'debug_disk_bytes': disk_bytes,
               'debug_apparent_bytes': apparent_bytes, 'executable_count': len(binaries),
               'largest_executables': [{'name': p.name, 'bytes': p.stat().st_size} for p in binaries[:5]]}
        result['runs'].append(row)
        (output / 'results.json').write_text(json.dumps(result, indent=2) + '\n')
        print(f'{name}: {seconds:.1f}s, {disk_bytes / 1024**3:.2f} GiB', flush=True)


if __name__ == '__main__':
    main()
