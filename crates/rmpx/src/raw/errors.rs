//! Internal error handling code, shared between encoding and decoding.
//!
//! This is used mainly for backwards compatibility and abstraction over std/no_std.

/// An alias to the "default" error handling type.
///
/// This is problematic when working with `#[no_std]`.
/// There is no [`std::error::Error`] trait and no [`std::io::Error`] type.
///
/// Furthermore, this doesn't abstract over the differences between implementations of
/// [`RmpRead`](crate::raw::decode::RmpRead) and [`RmpWrite`](crate::raw::encode::RmpWrite).
///
/// When working directly with bytes streams, the error type is actually
/// [Infallible](core::convert::Infallible).
///
/// For these two reasons, this type is deprecated
#[cfg(feature = "std")]
#[deprecated(
    note = "Doesn't abstract over RmpRead/RmpWrite (or work on no_std), use RmpRead::Error/RmpWrite::Error and \
            RmpReadErr/RmpWriteErr instead"
)]
pub type Error = ::std::io::Error;

#[cfg(not(feature = "std"))]
#[deprecated(note = "Doesn't work meaningfully on no_std")]
pub type Error = ::core::convert::Infallible;

/// Internal type used to abstract over the [`std::error::Error`] trait
///
/// This is a nop in no-std environments.
#[cfg(feature = "std")]
#[doc(hidden)]
pub trait MaybeErrBound: std::error::Error {}
#[cfg(feature = "std")]
impl<T: ?Sized + std::error::Error> MaybeErrBound for T {}
#[cfg(not(feature = "std"))]
#[doc(hidden)]
pub trait MaybeErrBound {}
#[cfg(not(feature = "std"))]
impl<T: ?Sized> MaybeErrBound for T {}
