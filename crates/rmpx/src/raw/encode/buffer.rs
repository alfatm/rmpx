//! Implementation of the [ByteBuf] type

use super::RmpWrite;
use alloc::vec::Vec;
#[cfg(not(feature = "std"))]
use core::fmt::{self, Display, Formatter};

/// An error returned from writing to `&mut [u8]` (a byte buffer of fixed capacity) on no_std
///
/// In feature="std", capacity overflow in `<&mut [u8] as std::io::Write>::write_exact()`
/// currently returns [`ErrorKind::WriteZero`](https://doc.rust-lang.org/std/io/enum.ErrorKind.html#variant.WriteZero).
///
/// Since no_std doesn't have std::io::Error we use this instead ;)
///
/// This is specific to `#[cfg(not(feature = "std"))]` so it is `#[doc(hidden)]`
#[derive(Debug)]
#[cfg(not(feature = "std"))]
#[doc(hidden)]
pub struct FixedBufCapacityOverflow {
    _priv: (),
}

/// An error returned from writing to `&mut [u8]`
///
/// Aliased for compatibility with `no_std` mode.
#[cfg(feature = "std")]
#[doc(hidden)]
pub type FixedBufCapacityOverflow = std::io::Error;

#[cfg(not(feature = "std"))]
impl Display for FixedBufCapacityOverflow {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        // This is intentionally vauge because std::io::Error is
        // Doesn't make sense for no_std to have bettetr errors than std
        f.write_str("Capacity overflow for fixed-size byte buffer")
    }
}
#[cfg(not(feature = "std"))]
impl crate::raw::encode::RmpWriteErr for FixedBufCapacityOverflow {}

/// Fallback implementation for fixed-capacity buffers
///
/// Only needed for no-std because we don't have
/// the blanket impl for `std::io::Write`
#[cfg(not(feature = "std"))]
impl RmpWrite for &mut [u8] {
    type Error = FixedBufCapacityOverflow;

    #[inline]
    fn write_bytes(&mut self, buf: &[u8]) -> Result<(), Self::Error> {
        // Taking the slice out of `self` gives the halves its full lifetime, so the rest can be stored back.
        match core::mem::take(self).split_at_mut_checked(buf.len()) {
            Some((head, rest)) => {
                head.copy_from_slice(buf);
                *self = rest;
                Ok(())
            }
            None => Err(FixedBufCapacityOverflow { _priv: () }),
        }
    }
}

/// A wrapper around `Vec<u8>` to serialize more efficiently.
///
/// This has a specialized implementation of `RmpWrite`
/// It gives `std::convert::Infailable` for errors.
/// This is because writing to `Vec<T>` can only fail due to allocation.
///
/// This has the additional benefit of working on `#[no_std]`
///
/// See also [serde_bytes::ByteBuf](https://docs.rs/serde_bytes/0.11/serde_bytes/struct.ByteBuf.html)
#[derive(Debug, Clone, Default, Eq, PartialEq, Hash, Ord, PartialOrd)]
pub struct ByteBuf {
    bytes: Vec<u8>,
}
impl ByteBuf {
    /// Construct a new empty buffer
    #[inline]
    pub fn new() -> Self {
        ByteBuf { bytes: Vec::new() }
    }
    /// Construct a new buffer with the specified capacity
    ///
    /// See [Vec::with_capacity] for details
    #[inline]
    pub fn with_capacity(capacity: usize) -> Self {
        ByteBuf {
            bytes: Vec::with_capacity(capacity),
        }
    }
    /// Unwrap the underlying buffer of this vector
    #[inline]
    pub fn into_vec(self) -> Vec<u8> {
        self.bytes
    }
    /// Wrap the specified vector as a [ByteBuf]
    #[inline]
    pub fn from_vec(bytes: Vec<u8>) -> Self {
        ByteBuf { bytes }
    }
    /// Get a reference to this type as a [Vec]
    #[inline]
    pub fn as_vec(&self) -> &Vec<u8> {
        &self.bytes
    }
    /// Get a mutable reference to this type as a [Vec]
    #[inline]
    pub fn as_mut_vec(&mut self) -> &mut Vec<u8> {
        &mut self.bytes
    }
    /// Get a reference to this type as a slice of bytes (`&[u8]`)
    #[inline]
    pub fn as_slice(&self) -> &[u8] {
        &self.bytes
    }
}
impl AsRef<[u8]> for ByteBuf {
    fn as_ref(&self) -> &[u8] {
        &self.bytes
    }
}
impl AsRef<Vec<u8>> for ByteBuf {
    #[inline]
    fn as_ref(&self) -> &Vec<u8> {
        &self.bytes
    }
}
impl AsMut<Vec<u8>> for ByteBuf {
    #[inline]
    fn as_mut(&mut self) -> &mut Vec<u8> {
        &mut self.bytes
    }
}
impl From<ByteBuf> for Vec<u8> {
    #[inline]
    fn from(buf: ByteBuf) -> Self {
        buf.bytes
    }
}
impl From<Vec<u8>> for ByteBuf {
    #[inline]
    fn from(bytes: Vec<u8>) -> Self {
        ByteBuf { bytes }
    }
}

impl RmpWrite for ByteBuf {
    type Error = core::convert::Infallible;

    #[inline]
    fn write_u8(&mut self, val: u8) -> Result<(), Self::Error> {
        self.bytes.push(val);
        Ok(())
    }

    #[inline]
    fn write_bytes(&mut self, buf: &[u8]) -> Result<(), Self::Error> {
        self.bytes.extend_from_slice(buf);
        Ok(())
    }
}
#[cfg(not(feature = "std"))]
impl RmpWrite for Vec<u8> {
    type Error = core::convert::Infallible;

    #[inline]
    fn write_u8(&mut self, val: u8) -> Result<(), Self::Error> {
        self.push(val);
        Ok(())
    }

    #[inline]
    fn write_bytes(&mut self, buf: &[u8]) -> Result<(), Self::Error> {
        self.extend_from_slice(buf);
        Ok(())
    }
}
