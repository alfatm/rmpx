//! The low-level API: readers and writers for single MessagePack values, with direct mapping to the binary
//! format. It makes no heap allocations and, without the `std` feature, works on byte slices in `no_std` builds.
//!
//! ## Detailed
//!
//! This module is the very basic functionality needed to work with MessagePack format, the base the serde
//! support and [`Value`](crate::value::Value) are built on.
//!
//! Currently there are two large modules: encode and decode. More detail you can find in the
//! corresponding sections.
//!
//! Formally every MessagePack message consists of some marker encapsulating a data type and the
//! data itself. Sometimes there are no separate data chunk, for example for booleans. In these
//! cases a marker contains the value. For example, the `true` value is encoded as `0xc3`.
//!
//! ```
//! let mut buf = Vec::new();
//! rmpx::raw::encode::write_bool(&mut buf, true).unwrap();
//!
//! assert_eq!([0xc3], buf[..]);
//! ```
//!
//! Sometimes a single value can be encoded in multiple ways. For example a value of `42` can be
//! represented as: `[0x2a], [0xcc, 0x2a], [0xcd, 0x00, 0x2a]` and so on, and all of them are
//! considered as valid representations. To allow fine-grained control over encoding such values
//! the library provides direct mapping functions.
//!
//! ```
//! let mut bufs = vec![vec![]; 5];
//!
//! rmpx::raw::encode::write_pfix(&mut bufs[0], 42).unwrap();
//! rmpx::raw::encode::write_u8(&mut bufs[1], 42).unwrap();
//! rmpx::raw::encode::write_u16(&mut bufs[2], 42).unwrap();
//! rmpx::raw::encode::write_u32(&mut bufs[3], 42).unwrap();
//! rmpx::raw::encode::write_u64(&mut bufs[4], 42).unwrap();
//!
//! assert_eq!([0x2a], bufs[0][..]);
//! assert_eq!([0xcc, 0x2a], bufs[1][..]);
//! assert_eq!([0xcd, 0x00, 0x2a], bufs[2][..]);
//! assert_eq!([0xce, 0x00, 0x00, 0x00, 0x2a], bufs[3][..]);
//! assert_eq!([0xcf, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x2a], bufs[4][..]);
//! ```
//!
//! But they aren't planned to be widely used. Instead we often need to encode bytes compactly to
//! save space. In these cases there are functions that guarantee that for encoding the most
//! compact representation will be chosen.
//!
//! ```
//! let mut buf = Vec::new();
//!
//! rmpx::raw::encode::write_sint(&mut buf, 300).unwrap();
//!
//! assert_eq!([0xcd, 0x1, 0x2c], buf[..]);
//! ```
//!
//! On the other hand for deserialization it is not matter in which representation the value is
//! encoded - the readers accept all of them.
//!
//! Sometimes you know the exact type representation and want to enforce the deserialization process
//! to make it strongly type safe.
//!
//! ```
//! let buf = [0xcd, 0x1, 0x2c];
//!
//! assert_eq!(300, rmpx::raw::decode::read_u16(&mut &buf[..]).unwrap());
//! ```
//!
//! However if you try to decode such bytearray as other integer type, for example `u32`, there will
//! be type mismatch error.
//!
//! ```
//! let buf = [0xcd, 0x1, 0x2c];
//! rmpx::raw::decode::read_u32(&mut &buf[..]).err().unwrap();
//! ```
//!
//! But sometimes all you want is just to encode an integer that *must* fit in the specified type
//! no matter how it was encoded. [`read_int`](decode::read_int) does that, to ease integration with
//! other MessagePack libraries.
//!
//! ```
//! let buf = [0xcd, 0x1, 0x2c];
//!
//! assert_eq!(300i16, rmpx::raw::decode::read_int(&mut &buf[..]).unwrap());
//! assert_eq!(300i32, rmpx::raw::decode::read_int(&mut &buf[..]).unwrap());
//! assert_eq!(300i64, rmpx::raw::decode::read_int(&mut &buf[..]).unwrap());
//! assert_eq!(300u16, rmpx::raw::decode::read_int(&mut &buf[..]).unwrap());
//! assert_eq!(300u32, rmpx::raw::decode::read_int(&mut &buf[..]).unwrap());
//! assert_eq!(300u64, rmpx::raw::decode::read_int(&mut &buf[..]).unwrap());
//! ```
//!
//! ## API
//!
//! Almost all API are represented as pure functions, which accepts a generic `Write` or `Read` and
//! the value to be encoded/decoded. For example let's do a round trip for π number.
//!
//! ```
//! let pi = std::f64::consts::PI;
//! let mut buf = Vec::new();
//! rmpx::raw::encode::write_f64(&mut buf, pi).unwrap();
//!
//! assert_eq!([0xcb, 0x40, 0x9, 0x21, 0xfb, 0x54, 0x44, 0x2d, 0x18], buf[..]);
//! assert_eq!(pi, rmpx::raw::decode::read_f64(&mut &buf[..]).unwrap());
//! ```
//!
pub mod decode;
pub mod encode;
mod errors;
mod marker;

pub use crate::raw::marker::Marker;

/// Version of the MessagePack [spec](http://github.com/msgpack/msgpack/blob/master/spec.md).
pub const MSGPACK_VERSION: u32 = 5;
