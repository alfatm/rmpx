# Changelog

## 1.0.0

First release of `rmpx`, a reworked fork of `rmp`, `rmp-serde` and `rmpv` from
[3Hren/msgpack-rust](https://github.com/3Hren/msgpack-rust) at `f4ad0d0` (May 2023, the 1.1.x era of those crates).
The wire format is unchanged: data written by the original crates reads with rmpx and the other way round. The Rust API
is not compatible, which is why this is a separate crate. The changelogs of the original crates are kept in
[docs/upstream-changelogs](docs/upstream-changelogs).

### One crate instead of three

| Before | Now |
|---|---|
| `rmp_serde::{to_vec, to_vec_named, from_slice, from_read}` | `rmpx::{to_vec, to_vec_named, from_slice, from_read}` |
| `rmp_serde::encode::write`, `write_named` | `rmpx::to_writer`, `to_writer_named` |
| `rmp_serde::encode::*` | `rmpx::ser::*` (`Serializer` also at the root) |
| `rmp_serde::decode::*` | `rmpx::de::*` (`Deserializer` also at the root) |
| `rmp_serde::config::*` | `rmpx::config::*` |
| `rmp_serde::{Raw, RawRef}` | `rmpx::{Raw, RawRef}` |
| `rmp_serde::MSGPACK_EXT_STRUCT_NAME`, `rmpv::MSGPACK_EXT_STRUCT_NAME` | `rmpx::MSGPACK_EXT_STRUCT_NAME` |
| `rmpv::*` (`Value`, `ValueRef`, `Integer`, `Utf8String`, ...) | `rmpx::value::*` (`Value`, `ValueRef` also at the root) |
| `rmpv::{decode, encode, ext}` | `rmpx::value::{decode, encode, ext}` |
| `rmp::{encode, decode, Marker, MSGPACK_VERSION}` | `rmpx::raw::{encode, decode, Marker, MSGPACK_VERSION}` |
| `rmp` feature `std` | feature `std` |
| `rmpv` feature `with-serde` | features `serde` + `value` |

The features `std`, `serde` and `value` are on by default. With `default-features = false` only `rmpx::raw` is left,
and it builds without `std`.

### Errors instead of panics

- `raw::encode::ValueWriteError` has a new variant, `OutOfRange`: the value does not fit the format it is written in.
- `write_pfix` and `write_nfix` return `Result<(), ValueWriteError<_>>` and give `OutOfRange` for a value outside
  their range; they used to assert.
- `write_str` and `write_bin` give `OutOfRange` for data longer than `u32::MAX` bytes, see Fixed below.

### Removed

Items the original crates kept only for compatibility between themselves:

- `rmp_serde::from_read_ref`: use `from_slice`.
- `Raw::from_utf8`, `RawRef::from_utf8`, already deprecated as removed functionality: a `Raw` holding invalid UTF-8
  comes from deserialization.
- `rmp::decode::read_str_ref`: use `read_str_from_slice`.
- `rmp::decode::read_data_*`: use the methods of `RmpRead`.
- The nightly `#[bench]` benches of the original crates: the criterion benches in `crates/rmpx-bench` cover them.

### Fixed

Five bugs of the original crates, found while reworking them: in code review, and by the stricter lints, which flag
every place that can panic.

- **Corrupt output for data over 4 GiB.** Strings, binaries, arrays and maps longer than `u32::MAX` were written with a
  silently truncated length header, so the output decoded as different data. The serializer and the `Value` writers
  now return an error, and `raw::encode::write_str` / `write_bin` return `ValueWriteError::OutOfRange`.
- **Panic on truncated input in `read_str_ref`.** It sliced the input by the announced string length without checking
  it, so a string header announcing more bytes than were left panicked. Untrusted input could trigger it. The function
  is removed in favor of `read_str_from_slice`, and the remaining string readers check every length.
- **No depth limit with `Deserializer::set_max_depth(0)`.** The depth counter was decremented before the check and
  underflowed: a panic in debug builds and effectively unlimited nesting in release builds. It is now a limit like any
  other.
- **Panics on out-of-order calls from `Serialize` / `Deserialize` impls.** `value::ext::to_value` panicked when a map
  value came before its key, and deserializing an ext value hit `unreachable!` when it was driven past its tag and data
  (in both the serde deserializer and `value::ext`). These return errors now.
- **Wrong read in `read_str_data` with a buffer of another length.** It read as many bytes as the buffer held instead of
  the string length; only debug builds asserted. It now reads the string length into the start of the buffer and returns
  `BufferSizeTooSmall` for a shorter buffer.

### Safety

- The crate is `#![forbid(unsafe_code)]`; the one `unsafe` block of the original, in the `no_std` writer for
  `&mut [u8]`, is replaced by safe code.
- Library code is denied every panic source Clippy can lint: `unwrap`, `expect`, `panic!`, `unreachable!`, `todo!`,
  indexing and slicing, string slicing, assertions in functions that return `Result`, and arithmetic that can overflow.

### Faster

Arrays and maps are preallocated when decoding `Value`s, at most 4 KiB per nesting level: the announced length comes
from the input, and the cap keeps the protection the originals had by not preallocating at all ([upstream
#151](https://github.com/3Hren/msgpack-rust/issues/151)), so a few bytes of input still cannot make the decoder reserve
much memory. Also faster `Value` decoding and encoding, faster `#[serde(flatten)]` encoding through a thread-local
buffer pool, exact reads in `from_read`, `Marker` conversions as plain casts, and the useful upstream changes made after
the fork. Numbers, the method and the trade-offs are in [docs/performance.md](docs/performance.md).

### Requirements

- Rust 1.88 or later, stable. Nightly is only used to build the wasm benches.
