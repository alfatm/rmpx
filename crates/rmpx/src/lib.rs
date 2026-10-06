//! Pure Rust [MessagePack](https://msgpack.org): serde support, a dynamic value type and a low-level API in one crate.
//!
//! A reworked fork of `rmp`, `rmp-serde` and `rmpv`: one crate, no `unsafe`, no panics, faster; the wire format is the
//! same, the API is not. The [README](https://github.com/alfatm/rmpx#readme) lists the differences and the trade-offs.
//!
//! - The crate root and [`ser`] / [`de`]: any `serde::Serialize` / `serde::Deserialize` type to and from MessagePack
//!   (feature `serde`).
//! - [`value`]: [`Value`] and [`ValueRef`], MessagePack data of any shape, and their readers and writers (feature
//!   `value`).
//! - [`raw`]: readers and writers for single MessagePack values with direct mapping to the binary format, no heap
//!   allocations; the only part available without `std`.
//!
//! All three features are on by default; `default-features = false` leaves only [`raw`], usable in `no_std` builds.
//!
//! # Serde
//!
//! ```
//! use serde::{Deserialize, Serialize};
//!
//! #[derive(Debug, PartialEq, Deserialize, Serialize)]
//! struct Human {
//!     age: u32,
//!     name: String,
//! }
//!
//! let john = Human { age: 42, name: "John".into() };
//!
//! // Structs as arrays of their fields: compact, but both sides must agree on the field order.
//! let buf = rmpx::to_vec(&john).unwrap();
//! assert_eq!(buf, [0x92, 0x2a, 0xa4, b'J', b'o', b'h', b'n']);
//!
//! // Structs as maps with field names, readable by any MessagePack implementation.
//! let named = rmpx::to_vec_named(&john).unwrap();
//!
//! // Either form decodes into the struct.
//! assert_eq!(rmpx::from_slice::<Human>(&buf).unwrap(), john);
//! assert_eq!(rmpx::from_slice::<Human>(&named).unwrap(), john);
//! ```
//!
//! [`Serializer`] and [`Deserializer`] work with any `std::io::Write` / `std::io::Read` and take their options
//! through [`config`].
//!
//! # Dynamic values
//!
//! ```
//! use rmpx::Value;
//!
//! let buf = rmpx::to_vec_named(&std::collections::BTreeMap::from([("id", 7)])).unwrap();
//!
//! let value = rmpx::value::decode::read_value(&mut &buf[..]).unwrap();
//! assert_eq!(value, Value::Map(vec![("id".into(), 7.into())]));
//! ```
//!
//! # Low level
//!
//! ```
//! let mut buf = Vec::new();
//! rmpx::raw::encode::write_sint(&mut buf, 300).unwrap();
//! assert_eq!(buf, [0xcd, 0x01, 0x2c]);
//! assert_eq!(rmpx::raw::decode::read_int::<u16, _>(&mut &buf[..]).unwrap(), 300);
//! ```
#![cfg_attr(not(feature = "std"), no_std)]
#![forbid(unsafe_code)]

extern crate alloc;

// Only serde support for `Value` uses it; Cargo cannot enable a dependency for a combination of features.
#[cfg(all(feature = "serde", not(feature = "value")))]
use serde_bytes as _;

#[cfg(feature = "serde")]
pub mod config;
#[cfg(feature = "serde")]
pub mod de;
pub mod raw;
#[cfg(feature = "serde")]
mod raw_string;
#[cfg(feature = "serde")]
pub mod ser;
#[cfg(feature = "value")]
pub mod value;

#[cfg(feature = "serde")]
pub use crate::de::{Deserializer, from_read, from_slice};
#[cfg(feature = "serde")]
pub use crate::raw_string::{Raw, RawRef};
#[cfg(feature = "serde")]
pub use crate::ser::{Serializer, to_vec, to_vec_named, to_writer, to_writer_named};
#[cfg(feature = "value")]
pub use crate::value::{Value, ValueRef};

/// Name of the serde newtype struct that stands for a MessagePack ext value.
///
/// MessagePack `ext(tag, data)` maps to the serde data model as `_ExtStruct((tag, data))`, so a type serializes as an
/// ext value like this:
///
/// ```
/// # #[cfg(all(feature = "serde", feature = "value"))] {
/// use serde::{Deserialize, Serialize};
///
/// #[derive(Debug, PartialEq, Serialize, Deserialize)]
/// #[serde(rename = "_ExtStruct")]
/// struct ExtStruct((i8, serde_bytes::ByteBuf));
///
/// let ext = ExtStruct((2, serde_bytes::ByteBuf::from(vec![5])));
/// let buf = rmpx::to_vec(&ext).unwrap();
/// assert_eq!(rmpx::value::decode::read_value(&mut &buf[..]).unwrap(), rmpx::Value::Ext(2, vec![5]));
/// assert_eq!(rmpx::from_slice::<ExtStruct>(&buf).unwrap(), ext);
/// # }
/// ```
pub const MSGPACK_EXT_STRUCT_NAME: &str = "_ExtStruct";

/// The examples in the README, compiled and run as doc tests.
#[cfg(doctest)]
#[doc = include_str!("../../../README.md")]
struct ReadmeDoctests;
