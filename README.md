# rmpx

[![crates.io](https://img.shields.io/crates/v/rmpx.svg)](https://crates.io/crates/rmpx)
[![docs.rs](https://docs.rs/rmpx/badge.svg)](https://docs.rs/rmpx)

Pure Rust [MessagePack](https://msgpack.org): serde support, a dynamic `Value` type and a low-level `no_std` API in one
crate.

**rmpx is a reworked fork** of [`rmp`](https://crates.io/crates/rmp), [`rmp-serde`](https://crates.io/crates/rmp-serde)
and [`rmpv`](https://crates.io/crates/rmpv) from [3Hren/msgpack-rust](https://github.com/3Hren/msgpack-rust), taken at
`f4ad0d0` (May 2023). The wire format is the same, so data written by one reads with the other. The Rust API is not:
the three crates became one, and parts of the API were renamed or removed. That is why this is a separate repository
and a separate crate rather than a new version of the originals.

## Usage

```toml
[dependencies]
rmpx = "1"
```

```rust
use serde::{Deserialize, Serialize};

#[derive(Debug, PartialEq, Serialize, Deserialize)]
struct Point {
    x: i32,
    y: i32,
}

let point = Point { x: 1, y: -2 };

// Structs as maps with field names, readable by any MessagePack implementation.
let buf = rmpx::to_vec_named(&point).unwrap();
assert_eq!(rmpx::from_slice::<Point>(&buf).unwrap(), point);

// Structs as arrays: more compact, but both sides must agree on the field order.
let compact = rmpx::to_vec(&point).unwrap();
assert_eq!(compact, [0x92, 0x01, 0xfe]);

// Data of any shape.
let value = rmpx::value::decode::read_value(&mut &buf[..]).unwrap();
assert_eq!(value["x"], rmpx::Value::from(1));
```

| Feature | Default | What it adds |
|---|---|---|
| `std` | yes | `std::io` readers and writers |
| `serde` | yes | `to_vec`, `from_slice`, `Serializer`, `Deserializer` (`rmpx::ser`, `rmpx::de`, `rmpx::config`) |
| `value` | yes | `Value` and `ValueRef` with their readers and writers (`rmpx::value`); with `serde`, serde support for them |

With `default-features = false` only `rmpx::raw` is left: readers and writers for single values that work on byte
slices without `std` or allocation.

## How it differs from rmp, rmp-serde and rmpv

### One crate

| Before | rmpx |
|---|---|
| `rmp_serde::{to_vec, to_vec_named, from_slice, from_read}` | `rmpx::{to_vec, to_vec_named, from_slice, from_read}` |
| `rmp_serde::encode::write`, `write_named` | `rmpx::to_writer`, `to_writer_named` |
| `rmp_serde::encode`, `decode`, `config` | `rmpx::ser`, `rmpx::de`, `rmpx::config` |
| `rmpv::*` | `rmpx::value::*` (`Value`, `ValueRef` also at the root) |
| `rmp::*` | `rmpx::raw::*` |

One version and one dependency instead of three that have to match, no `encode` / `decode` modules with the same names
in different crates, and a dependency cycle between `rmpv`'s tests and `rmp-serde` is gone. The full list, including
the removed deprecated functions, is in the [changelog](CHANGELOG.md).

### No `unsafe`, no panics

- The crate is `#![forbid(unsafe_code)]`. The one `unsafe` block of the original (the `no_std` writer for `&mut [u8]`)
  is replaced by safe code.
- Library code is linted against every panic source Clippy can see: `unwrap`, `expect`, `panic!`, indexing and
  slicing, `unreachable!`, assertions in functions that return `Result`, string slicing, and arithmetic that can
  overflow (a panic in debug builds).
- Functions that used to assert now return errors: `write_pfix` / `write_nfix` with a value outside their range, and
  `write_str` / `write_bin` with data longer than `u32::MAX` bytes, return `ValueWriteError::OutOfRange`.

Reworking the crates under these rules turned up five bugs in the originals, all fixed (details in the
[changelog](CHANGELOG.md#fixed)):

1. Data longer than `u32::MAX` was written with a silently truncated length, producing corrupt output.
2. `read_str_ref` panicked on truncated input, which untrusted input could trigger.
3. `Deserializer::set_max_depth(0)` underflowed the depth counter: a panic in debug builds, no depth limit in release.
4. `Serialize` / `Deserialize` impls that call the serializer or deserializer out of order caused panics.
5. `read_str_data` read the wrong number of bytes when given a buffer of a different length.

Allocation failure still aborts, as everywhere in Rust's standard collections.

### Faster

Measured against the fork point, on the same machine, each result confirmed on two code layouts; details, method and
everything that was tried and dropped are in [docs/performance.md](docs/performance.md).

| | Native (x86-64) | Wasm (V8) |
|---|---:|---:|
| `Value` decoding, 28 MB real document | −30% | −19…23% |
| `from_slice::<Value>` (any `deserialize_any` visitor), 28 MB | | −25…28% |
| `Value` encoding, 28 MB | −26…29% | −13…16% |
| Many `#[serde(flatten)]` structs in one message | −26…40% | −13…16% |
| `from_read` into a large struct | −12…17% | −18…19% |
| `from_slice` into structs | −4…13% | −9…19% |

The main changes: arrays and maps are preallocated when decoding `Value`s, buffers for maps of unknown length
(`#[serde(flatten)]`) come from a thread-local pool, `from_read` reads known lengths in one call, `Marker` converts to
and from bytes as a plain cast, and the useful upstream changes made after the fork are ported.

### Trade-offs

- **Breaking API.** Migrating from the original crates means changing imports, see the table above.
- **`to_vec_named` of a large struct is 7-9% slower natively.** About 3% is the length check that prevents the
  truncation bug, the rest is code placement. Other encoding benches are unchanged or faster.
- **Memory kept per thread.** The flatten buffer pool keeps up to 16 buffers of at most 64 KiB on each thread that
  serializes such structs, at most 1 MiB, usually far less.
- **Preallocation is capped.** Arrays and maps are reserved up front only up to 4 KiB per nesting level and grow beyond
  that, because the announced length comes from the input: a few bytes of it must not make the decoder reserve much
  memory ([upstream #151](https://github.com/3Hren/msgpack-rust/issues/151)). The originals did not preallocate at all.
- **Rust 1.88 or later**, stable (edition 2024).
- **Some later upstream changes are left out** on purpose, for example writing `Vec<u8>` as binary without
  `serde_bytes` and reporting allocation failure from `to_vec` (it cost 20-45% on encoding). Each is listed with the
  reason in [docs/performance.md](docs/performance.md#upstream-changes-since-the-fork).

### Wasm

`just bench-wasm` runs the benches compiled for wasm on V8. Size-optimized builds (`opt-level = "s"`) do not inline the
write path; giving `opt-level = 3` to the crate that calls `to_vec` / `from_slice` made encoding 45-77% faster in the
benches. See [docs/performance.md](docs/performance.md#wasm-builds).

## Development

Everything builds on stable Rust: `just lint` (rustfmt, Clippy, cargo-deny, cargo-sort, cargo-machete, builds with the
lowest allowed dependency versions and with the oldest supported Rust), `just test`, `just bench`, and
`just bench-ab <git ref>` to compare the working tree with a commit. Only `just bench-wasm` uses a nightly toolchain,
for `build-std`; it installs the one it needs.

## License

MIT. Copyright (c) 2017 Evgeny Safronov, the author of the original crates, and (c) 2026 Andrew Panin.
