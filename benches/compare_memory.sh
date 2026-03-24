#!/usr/bin/env bash
# Compare memory usage: xbrl-rs (Rust) vs Arelle (Python)
#
# Usage:
#   ./benches/compare_memory.sh

set -euo pipefail
cd "$(dirname "$0")/.."

separator() {
    echo ""
    echo "================================================================"
    echo "$1"
    echo "================================================================"
}

separator "Building Rust binary (release, system allocator)"
cargo build --release --bin bench_memory

separator "Rust: /usr/bin/time peak RSS"
/usr/bin/time -v target/release/bench_memory

separator "Python/Arelle: /usr/bin/time peak RSS"
/usr/bin/time -v uv run benches/bench_memory.py
