set export
set dotenv-load
set ignore-comments

PWD := invocation_directory()

PROFILE := env_var_or_default("PROFILE", "release")

# the nightly for `bench-wasm` (build-std), the rest builds on stable
NIGHTLY := env_var_or_default("NIGHTLY", "nightly-2026-09-26")

default:
    @just --list

# everything on stable, nightly is only for `bench-wasm`
lint:
    cargo deny --log-level error check advisories bans sources
    cargo fmt --all --check
    # clippy below covers the default build, check the other feature sets separately
    cargo check -p rmpx --no-default-features
    cargo check -p rmpx --no-default-features --features serde
    cargo check -p rmpx --no-default-features --features value
    @just minimal-versions
    @just msrv
    cargo workspace-lints
    cargo clippy --profile "$PROFILE" --all-features --all-targets
    cargo sort -c -w
    cargo machete

# rmpx must build with the lowest versions of its dependencies that its manifest allows; checked in a scratch copy of
# the workspace, so the lockfile here is left alone
minimal-versions:
    #!/usr/bin/env bash
    set -euo pipefail
    dir=target/minimal-versions
    rm -rf "$dir" && mkdir -p "$dir"
    cp -r Cargo.toml crates .cargo "$dir"/
    cd "$dir"
    cargo generate-lockfile -q
    for dep in $(cargo metadata --format-version 1 --no-deps | jq -r '.packages[] | select(.name == "rmpx") | .dependencies[] | select(.kind == null and .source != null) | .name'); do
        version=$(sed -n "s/^$dep = { version = \"\([^\"]*\)\".*/\1/p" Cargo.toml)
        cargo update -q -p "$dep" --precise "$version"
    done
    cargo check -q -p rmpx --all-features

# the oldest Rust the manifest promises
msrv:
    cargo +$(sed -n 's/^rust-version = "\(.*\)"/\1/p' Cargo.toml) check -q -p rmpx --all-features

fix:
    cargo clippy --fix --allow-dirty --allow-staged --all-features --all-targets
    cargo fmt --all
    cargo sort -w
    cargo machete --fix

test *args='':
    cargo nextest run --run-ignored default {{ if PROFILE == "release" { "--release" } else { "" } }} $args
    cargo test --doc

test-integration *args='':
    cargo nextest run --run-ignored ignored-only {{ if PROFILE == "release" { "--release" } else { "" } }} $args

test-all *args='':
    @just test $args
    @just test-integration $args

# RMPX_BENCH_FILES=/abs/a.msgpack:/abs/b.msgpack adds real files (benches run from their package dir, so absolute paths),
# pass `-- --save-baseline before` / `-- --baseline before` to compare runs
bench *args='':
    cargo bench -p rmpx-bench $args

# the same benches built for wasm (MVP features, size-optimized, wasm-opt -O3) and run on V8 in node, see
# scripts/wasi-run.mjs; needs `rustup target add wasm32-wasip1`, node and wasm-opt
bench-wasm *args='':
    rustup toolchain install -q "$NIGHTLY" --profile minimal --component rust-src --target wasm32-wasip1
    cargo +"$NIGHTLY" bench -Zbuild-std=std,panic_abort --target wasm32-wasip1 --profile wasm -p rmpx-bench $args

# A/B of a git ref against the working tree, see scripts/bench-ab.sh; BENCH_WASM=1 compares the wasm builds
bench-ab ref='HEAD' filter='':
    ./scripts/bench-ab.sh "{{ref}}" "{{filter}}"
