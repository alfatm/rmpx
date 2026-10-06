#!/usr/bin/env bash
# A/B benchmark of a git ref (HEAD by default) against the working tree.
#
# Both builds run twice, alternately, pinned to one core and sharing one criterion dir. They alternate group by group
# (A, B, A, B for one bench group, then the next), so the runs compared with each other are seconds apart and background
# load or clock changes hit both sides alike. The difference between the two runs of the same build shows the noise, a
# change counts only when both runs agree.
#
# Code placement alone moves some benches by up to 2x on the same source, so confirm a result on a second layout:
# rerun with e.g. CARGO_PROFILE_RELEASE_CODEGEN_UNITS=1 and keep only what holds in both.
#
# Usage: scripts/bench-ab.sh [ref] [criterion filter]
# Env:   BENCH_CORE     core to pin to (default 2)
#        BENCH_TIME     measurement seconds per bench and run (default 2)
#        BENCH_WASM=1   compare the wasm builds instead, as `just bench-wasm` runs them (on $NIGHTLY, default
#                       nightly-2026-09-26)
#        RMPX_BENCH_FILES real files to bench, see crates/rmpx-bench/benches/codec.rs (relative paths are fine here)
#        CARGO_PROFILE_RELEASE_* overrides apply to both builds
# The ref must contain the bench crate: crates/rmpx-bench, or crates/rmp-bench from before the crates were merged.
set -euo pipefail

ROOT="$(git rev-parse --show-toplevel)"
REF="${1:-HEAD}"
FILTER="${2:-}"
OUT="$ROOT/target/bench-ab"
BASE="$OUT/base"
CORE="${BENCH_CORE:-2}"
TIME="${BENCH_TIME:-2}"
LABEL="$REF"
[[ "$REF" =~ ^[0-9a-f]{40}$ ]] && LABEL="${REF:0:10}"

export CRITERION_HOME="$OUT/criterion"
rm -rf "$CRITERION_HOME"

git -C "$ROOT" worktree remove --force "$BASE" 2>/dev/null || true
mkdir -p "$OUT"
git -C "$ROOT" worktree add --detach "$BASE" "$REF" >/dev/null
trap 'git -C "$ROOT" worktree remove --force "$BASE"' EXIT

# The bench package of a tree: rmpx-bench, or rmp-bench before the crates were merged into rmpx.
bench_package() {
    if [ -d "$1/crates/rmpx-bench" ]; then
        echo rmpx-bench
    elif [ -d "$1/crates/rmp-bench" ]; then
        echo rmp-bench
    fi
}
BASE_PKG="$(bench_package "$BASE")"
ROOT_PKG="$(bench_package "$ROOT")"
if [ -z "$BASE_PKG" ]; then
    echo "bench-ab: $REF has no bench crate" >&2
    exit 1
fi

# Cargo.lock is not committed: give the ref the same dependency versions as the working tree.
if [ -f "$ROOT/Cargo.lock" ]; then
    cp "$ROOT/Cargo.lock" "$BASE/Cargo.lock"
fi

# Benches run from their package dir, a different one for each tree: make the paths absolute.
# Older trees read the files from RMP_BENCH_FILES.
if [ -n "${RMPX_BENCH_FILES:-}" ]; then
    files=""
    IFS=: read -ra paths <<<"$RMPX_BENCH_FILES"
    for path in "${paths[@]}"; do
        [ -n "$path" ] && files="$files:$(realpath "$path")"
    done
    export RMPX_BENCH_FILES="${files#:}" RMP_BENCH_FILES="${files#:}"
fi

CARGO_ARGS=()
TOOLCHAIN=()
if [ "${BENCH_WASM:-0}" = 1 ]; then
    NIGHTLY="${NIGHTLY:-nightly-2026-09-26}"
    rustup toolchain install -q "$NIGHTLY" --profile minimal --component rust-src --target wasm32-wasip1
    TOOLCHAIN=(+"$NIGHTLY")
    CARGO_ARGS+=(-Zbuild-std=std,panic_abort --target wasm32-wasip1 --profile wasm)
else
    CARGO_ARGS+=(--profile bench)
fi

bench() {
    local dir="$1" target="$2" name="$3" filter="$4" package
    package="$(bench_package "$dir")"
    (cd "$dir" && CARGO_TARGET_DIR="$target" taskset -c "$CORE" cargo "${TOOLCHAIN[@]}" bench -q -p "$package" "${CARGO_ARGS[@]}" -- \
        --warm-up-time 1 --measurement-time "$TIME" --save-baseline "$name" "$filter" >/dev/null 2>"$OUT/$name.log") || {
        cat "$OUT/$name.log" >&2
        exit 1
    }
}

(cd "$BASE" && CARGO_TARGET_DIR="$OUT/target-base" cargo "${TOOLCHAIN[@]}" build -q -p "$BASE_PKG" "${CARGO_ARGS[@]}" --benches)
(cd "$ROOT" && cargo "${TOOLCHAIN[@]}" build -q -p "$ROOT_PKG" "${CARGO_ARGS[@]}" --benches)

# One exact regex per bench group (the id up to its first `/`), from the benches matching the filter.
mapfile -t groups < <(
    (cd "$ROOT" && cargo "${TOOLCHAIN[@]}" bench -q -p "$ROOT_PKG" "${CARGO_ARGS[@]}" -- --list 2>/dev/null) | python3 -c '
import re, sys
selected = re.compile(sys.argv[1])
groups = {}
for line in sys.stdin:
    if line.endswith(": benchmark\n"):
        bench = line[: -len(": benchmark\n")]
        if selected.search(bench):
            groups.setdefault(bench.split("/")[0], []).append(re.escape(bench))
for benches in groups.values():
    print("^(" + "|".join(benches) + ")$")
' "$FILTER"
)
if [ "${#groups[@]}" -eq 0 ]; then
    echo "bench-ab: no bench matches '$FILTER'" >&2
    exit 1
fi

for group in "${groups[@]}"; do
    bench "$BASE" "$OUT/target-base" A1 "$group"
    bench "$ROOT" "$ROOT/target" B1 "$group"
    bench "$BASE" "$OUT/target-base" A2 "$group"
    bench "$ROOT" "$ROOT/target" B2 "$group"
    echo "bench-ab: ${group:0:60}... done" >&2
done

python3 - "$CRITERION_HOME" "$LABEL" <<'EOF'
import glob, json, os, sys

root, ref = sys.argv[1], sys.argv[2]


def median(bench_dir, run):
    path = os.path.join(bench_dir, run, "estimates.json")
    with open(path) as f:
        return json.load(f)["median"]["point_estimate"]


def fmt(ns):
    for unit, div in (("ms", 1e6), ("µs", 1e3)):
        if ns >= div:
            return f"{ns / div:.2f} {unit}"
    return f"{ns:.1f} ns"


rows = []
for est in glob.glob(f"{root}/**/A1/estimates.json", recursive=True):
    bench_dir = os.path.dirname(os.path.dirname(est))
    try:
        a1, b1, a2, b2 = (median(bench_dir, run) for run in ("A1", "B1", "A2", "B2"))
    except FileNotFoundError:
        continue
    rows.append((os.path.relpath(bench_dir, root), (a1 + a2) / 2, (b1 + b2) / 2, b1 / a1 - 1, b2 / a2 - 1, a2 / a1 - 1))

print(f"| bench | {ref} | working tree | Δ run 1 | Δ run 2 | noise ({ref} run 2 vs 1) |")
print("|---|---:|---:|---:|---:|---:|")
for name, a, b, d1, d2, noise in sorted(rows):
    print(f"| {name} | {fmt(a)} | {fmt(b)} | {d1:+.1%} | {d2:+.1%} | {noise:+.1%} |")
EOF
