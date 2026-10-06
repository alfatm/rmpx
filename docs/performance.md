# Performance work in this fork

This fork of [3Hren/msgpack-rust](https://github.com/3Hren/msgpack-rust) was copied from upstream commit
`f4ad0d0` (2023-05-06). This page records what was changed for speed, what it bought, what was tried and dropped,
and which later upstream changes were ported or left out, with the reasons. None of it changes the wire format or the
API; the API changed separately, when the crates were merged (see the
[changelog](../crates/rmpx/CHANGELOG.md)).

The work was done while the code was still three crates, `rmp`, `rmp-serde` and `rmpv`, and the text below keeps those
names. They are now the modules [`raw`](../crates/rmpx/src/raw), [`ser`](../crates/rmpx/src/ser.rs) /
[`de`](../crates/rmpx/src/de.rs) and [`value`](../crates/rmpx/src/value) of the single `rmpx` crate.

## Results

Native, x86-64 (AMD Zen 5), thin LTO, the original fork point against this tree. The ranges are two alternating runs;
each row holds on both code layouts (see below), where the layouts disagree the row shows the second one.

| Bench | Default layout | `codegen-units=1` |
|---|---:|---:|
| `read_value`, 28 MB real document | −30% | −30% |
| `read_value_ref`, 28 MB real document | −30% | −32% |
| `write_value`, 28 MB real document | −28…29% | −26% |
| `read_value` / `read_value_ref`, 0.7 MB real document | −8% / −14% | −9% / −20% |
| `to_vec_named`, many `#[serde(flatten)]` structs | −26…34% | −37…40% |
| `from_read_named`, large struct | −12…14% | −15…17% |
| `from_slice_named` / `from_slice` into `IgnoredAny`, large struct | −5…7% / −4…7% | −10% / −13% |
| `read_value` / `read_value_ref`, synthetic tree | −6…7% / −7…8% | −7% / −13% |
| `to_vec_named`, large struct | +7…8% | +7…9% |
| `write_value` synthetic tree, websocket response stream | +3…4%, +2% | +3…8%, +4% |
| Single tiny websocket messages, encode and decode | within ±5% | within ±5% |

Not counted, because they flip with the layout: `decode_strs` −45% on one layout and −1% on the other, `to_vec` of a
large struct +13% and −1%, `from_slice` of the large document into `IgnoredAny` 0% and −13%.

Wasm (`just bench-wasm`: MVP features, `opt-level = "s"`, fat LTO, `wasm-opt -O3`, V8 in node), same comparison:

| Bench | Change |
|---|---:|
| `from_slice::<rmpv::Value>`, 28 MB / 0.7 MB real document | −25…28% / −30% |
| `read_value_ref` / `read_value`, 28 MB real document | −23% / −19% |
| `to_vec(&rmpv::Value)` / `write_value`, 28 MB real document | −14% / −13…16% |
| `from_slice` into structs (large struct, small message, websocket requests) | −9…19% |
| `decode_ints` | −62% |
| `to_vec` / `to_vec_named`, large struct | −5…8% / −3…4% |
| Tiny websocket responses, encode | −0…20% |

The `to_vec_named` regression is the price of the length checks (about 3%) plus code placement; `to_vec` without
field names shows none on the second layout.

## How it was measured

- Benches live in [`crates/rmpx-bench`](../crates/rmpx-bench): `codec` (primitives, a small message, a large profile,
  `#[serde(flatten)]` structs, `rmpv`, and real MessagePack files through `RMPX_BENCH_FILES`) and `websocket` (a stream
  of tiny messages shaped like a websocket game server's protocol). Run them with `just bench`.
- Comparisons use [`scripts/bench-ab.sh`](../scripts/bench-ab.sh) (`just bench-ab <ref>`): both builds run twice,
  alternately, pinned to one core, and the table shows the difference between the two runs of the same build as the
  noise.
- `just bench-wasm` builds the same benches for `wasm32-wasip1` and runs them on V8 through node's WASI
  ([`scripts/wasi-run.mjs`](../scripts/wasi-run.mjs)), with the settings of a module shipped to browsers: MVP
  features with `build-std`, `opt-level = "s"`, fat LTO, `wasm-opt -O3`, and the allocator std uses on
  `wasm32-unknown-unknown` (dlmalloc). `build-std` needs a nightly toolchain, the only part of the project that does.
  `memcpy` still comes from wasi-libc, which uses `memory.copy`, so large copies are faster than in a pure MVP build;
  A/B comparisons are unaffected. `BENCH_WASM=1 just bench-ab` compares wasm builds.
- Builds use `lto = "thin"`, a common release profile for servers. Without LTO, codegen-unit partitioning moved
  results by up to 20% and showed regressions that do not exist in production builds.
- Code placement alone moves some benches a lot: on the development machine (AMD Zen 5) the same source ran
  `from_slice` 1x to 2.4x slower depending on where the linker put it, and no build flag pins the fast layout. So
  every result was confirmed on a second layout (`CARGO_PROFILE_RELEASE_CODEGEN_UNITS=1`), and only changes that hold
  on both count. A difference under about 8% on one layout is noise: an A/B of identical code shows that much.

## What went in

**Preallocated arrays and maps in `rmpv`.** `read_value` and `read_value_ref` used to grow every array and map from
an empty `Vec`. They now reserve up front, at most 4 KiB per nesting level. The length comes from untrusted input,
and nested levels are alive at once, so the cap bounds what a tiny input can make the decoder reserve
([upstream #151](https://github.com/3Hren/msgpack-rust/issues/151)): a first version with a 64 KiB cap let 3 KB of
nested headers reserve 33-65 MB. `crates/rmpx/tests/value_serde_prealloc.rs` checks the bound with a counting allocator.

**A thread-local buffer pool for unknown-length sequences and maps.** A struct with a `#[serde(flatten)]` field is
serialized as a map of unknown length, so rmp-serde buffers its elements and writes the length first. Every such
struct allocated and freed a buffer. The buffers now come from a per-thread pool, buffers over 64 KiB are freed
instead of kept, and the pool holds at most 16. Encoding many flattened structs in one message got 26-40% faster.
A single tiny flattened message is no faster than before: one allocation from a warm allocator costs about as much
as the pool lookup.

**Exact reads in `rmp_serde::decode::ReadReader`.** `from_read` read every string and binary with
`take().read_to_end()`, growing the buffer. A known length up to 64 KiB is now read with one `read_exact`; longer
ones still grow while reading, so a fake length cannot make it allocate.

**Lengths over `u32::MAX` are rejected instead of truncated.** MessagePack stores lengths as `u32`, and longer
strings, binaries, arrays and maps used to get a truncated length header, which silently produced corrupt output.
rmp-serde and rmpv now return `InvalidDataWrite` with `io::ErrorKind::InvalidInput`. rmp's `write_str` and
`write_bin` cannot build such an error for a generic writer, so they `debug_assert!` and document that the caller
checks. The error is built in a `#[cold]`, `#[inline(never)]` function: building it inline bloated the encoders,
stopped them from being inlined and cost up to 25% on encoding. The check itself costs about 3% on encoding structs
with field names.

**Smaller changes.** `deserialize_any` handles one marker per arm without a catch-all, so a new marker cannot be
misread silently; the minimum dependency versions are precise again and `just lint` builds against them; tests and
docs use serde's `derive` feature instead of `serde_derive`.

## Tried and dropped

| Change | Why it was dropped |
|---|---|
| Write a marker and its payload with one `write_bytes` | 14-24% faster without LTO, but with thin LTO the two writes are inlined anyway and the combined one was slower |
| Serialize into `ByteBuf` instead of `Vec<u8>` | No gain: the serializer is built on `io::Write`, and `Vec<u8>`'s `write_all` already inlines to an infallible `extend_from_slice` |
| `#[inline]` to keep `from_slice` in its fast mode | The fast and slow modes come from code placement, not from an inlining decision: the hot function was identical in both builds |
| SIMD, including `simdutf8` | MessagePack is decoded sequentially, so only UTF-8 validation could use it, and profiles do not show it as a bottleneck |
| A buffer pool per `Serializer` | Superseded by the thread-local pool: `to_vec` creates a new serializer per message, so a per-serializer pool never reused anything for single messages |
| 64 KiB preallocation cap for arrays and maps | Let a tiny input reserve tens of MB through nesting, see #151 above |

## Upstream changes since the fork

Upstream has 37 commits after `f4ad0d0`, most of them a refactoring series from April 2024.

**Ported:**

| Upstream | What | Notes |
|---|---|---|
| `cf88001` | Drop `byteorder`, use `to_be_bytes` / `from_be_bytes` | `pastey` became unneeded too. The deprecated `rmp::decode::read_data_*` functions that upstream removed in the same commit were kept then and removed when the crates were merged |
| `17db0dd` | Drop `num-traits` from `rmpv` | Same results through `try_from` / `as`. rmp keeps it: `read_int<T: FromPrimitive>` is public API |
| `f9f02d8` | `Marker` is `repr(u8)` with each discriminant equal to its wire byte | Converting bytes and markers is close to a copy. `Marker` derives no `Ord`, so the new variant order changes no comparison. A test round-trips all 256 bytes |
| `454e0c5` | `rmpv` counts nesting depth in `u16` | Neutral in benches. The public functions keep `max_depth: usize`, a larger limit is clamped to `u16::MAX`, far beyond what any stack holds |

**Not ported:**

| Upstream | What | Why not |
|---|---|---|
| `508ac77` | Decode `bin` as a sequence when a sequence is asked for | A bug fix, but it changes behaviour: input that fails today would decode |
| `0b79883` | Decode `i128` / `u128` from smaller integers | New behaviour, nothing here needs it |
| `0992f9a` | One error text for all unexpected types in the ext serializer | Only lets the linker merge functions for binary size, and the error texts get less specific |
| `06414f5` | `to_vec` reports OOM as an error (`try_reserve` per write) and drops `with_capacity(128)` | Measured: +20-45% on encoding, and still +15-72% with the check moved to a cold path. Once the writer can fail, the compiler keeps the error handling at every write in the serializer. On Linux memory runs out through the OOM killer long before an allocation reports failure, so there is nothing to gain. Dropping `with_capacity(128)` alone would also add 4-5 reallocations per small message |
| `21b206c`, `9d3d6ec`, `159dd6d`, `8009539`, `6d256d4`, `c774144` | Runtime serializer config and less generic code | Saves code only when several serializer configs are used; a typical user decodes with one and encodes with one (`to_vec_named`). They rewrite the parts of `encode.rs` and `config.rs` changed here |
| `926682d` | Different structure for writing length headers | The same kind of change as the dropped single-write experiment above |
| `e43cd65` | `write_uint8` for `u8` values | LLVM already removes the extra comparisons when `write_uint(v as u64)` is inlined |
| `6dd81ee` | `BytesMode`: write `Vec<u8>` as `bin` without `serde_bytes` | A new feature, upstream calls it a hack that may break data types |
| `09eade5` | `MessageLen`: message length from a byte stream | Not needed, websocket frames already delimit messages |
| Clippy, README and version bumps | | Nothing to take |

Upstream still truncates lengths over `u32::MAX` (`1d9856b` moved the cast earlier), so the length fix, the #151
regression test and the flatten buffer pool are candidates to offer upstream.

## Wasm builds

Size-optimized builds (`opt-level = "s"` or `"z"`, the usual profile for wasm shipped to browsers) do not inline
`Vec::extend_from_slice`, so every write, even a single marker byte, costs a call for the capacity check and another
for a short `memcpy`. With `opt-level = 3` for the crates involved the wasm benches encode 45-77% faster and decode
10-18% faster, at the cost of a larger module.

The serializer and deserializer are generic, so they are compiled in the crate that calls `to_vec` or `from_slice`,
not in rmp-serde: a profile override for rmp-serde alone changes nothing. The override has to name the calling crate:

```toml
[profile.release.package.my-wasm-crate]
opt-level = 3
```

In a real wasm module that converts between MessagePack and JS values, most of the time goes to calls between wasm
and JS rather than to MessagePack itself (about 93% when decoding a 28 MB document into JS objects), so measure the
whole module before deciding.

## Known limits

- Deeply nested input (about 1000 levels) overflows a 2 MiB stack in debug builds before the depth limit stops it.
  Release builds stay within the limit. The same was true before these changes.
- Decoding messages that combine `#[serde(flatten)]` with an internally tagged enum (`#[serde(tag = "...")]`) is
  dominated by serde itself: it buffers the whole message into serde's `Content` first. That cost is outside this
  crate.
