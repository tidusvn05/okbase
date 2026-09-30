#!/usr/bin/env bash
# Chạy toàn bộ benchmark v2 tuần tự (tránh tranh CPU giữa các lần đo)
cd "$(dirname "$0")"
B=./target/release/embed-bench
run() { echo "=== $*"; env DATA=v2 "$@" 2>&1 | grep -vE '^\s*$' | grep -v '^bm25 ' | tail -4; }
echo "=== bm25"; DATA=v2 $B bm25 | grep bm25
for m in e5-small e5-base e5-large bge-m3 bge-m3-int8 gemma gemma-q gemma-q4 gte-mb gte-mb-int8 nomic-v2 qwen3-0.6b; do run $B $m; done
for m in e5-small gemma-q4 gte-mb-int8; do run RERANK=1 $B $m; done
for m in bge-m3 bge-m3-int8 gemma-q4 gte-mb-int8 e5-small; do run THREADS=2 $B $m; done
echo DONE
