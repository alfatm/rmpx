pub use crate::raw::encode::ValueWriteError as Error;
use std::io;

mod value;
mod value_ref;

pub use self::value::write_value;
pub use self::value_ref::write_value_ref;

/// MessagePack stores lengths as `u32`, a longer string, binary, array, map or ext can not be encoded.
#[inline]
fn len_u32(len: usize) -> Result<u32, Error> {
    u32::try_from(len).map_err(|_| len_overflow())
}

// Out of line, so the error construction does not bloat the encoders and stop them from being inlined.
#[cold]
#[inline(never)]
fn len_overflow() -> Error {
    Error::InvalidDataWrite(io::Error::new(
        io::ErrorKind::InvalidInput,
        "length exceeds the MessagePack limit of u32::MAX",
    ))
}
