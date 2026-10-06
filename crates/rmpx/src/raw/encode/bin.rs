use super::RmpWrite;
use crate::raw::Marker;
use crate::raw::encode::{ValueWriteError, write_marker};

/// Encodes and attempts to write the most efficient binary array length implementation to the given
/// write, returning the marker used.
///
/// This function is useful when you want to get full control for writing the data itself, for
/// example, when using non-blocking socket.
///
/// # Errors
///
/// This function will return `ValueWriteError` on any I/O error occurred while writing either the
/// marker or the data.
pub fn write_bin_len<W: RmpWrite>(wr: &mut W, len: u32) -> Result<Marker, ValueWriteError<W::Error>> {
    if len < 256 {
        write_marker(&mut *wr, Marker::Bin8)?;
        wr.write_data_u8(len as u8)?;
        Ok(Marker::Bin8)
    } else if len <= u16::MAX as u32 {
        write_marker(&mut *wr, Marker::Bin16)?;
        wr.write_data_u16(len as u16)?;
        Ok(Marker::Bin16)
    } else {
        write_marker(&mut *wr, Marker::Bin32)?;
        wr.write_data_u32(len)?;
        Ok(Marker::Bin32)
    }
}

/// Encodes and attempts to write the most efficient binary implementation to the given `Write`.
///
/// # Errors
///
/// This function will return `ValueWriteError` on any I/O error occurred while writing either the
/// marker or the data.
///
/// [`ValueWriteError::OutOfRange`] if the binary is longer than `u32::MAX` bytes, the longest MessagePack can encode.
pub fn write_bin<W: RmpWrite>(wr: &mut W, data: &[u8]) -> Result<(), ValueWriteError<W::Error>> {
    let len = u32::try_from(data.len()).map_err(|_| ValueWriteError::OutOfRange)?;
    write_bin_len(wr, len)?;
    wr.write_bytes(data).map_err(ValueWriteError::InvalidDataWrite)
}
