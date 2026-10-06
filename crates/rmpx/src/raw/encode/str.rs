use super::{RmpWrite, write_marker};
use crate::raw::Marker;
use crate::raw::encode::ValueWriteError;

/// Encodes and attempts to write the most efficient string length implementation to the given
/// write, returning the marker used.
///
/// # Errors
///
/// This function will return `ValueWriteError` on any I/O error occurred while writing either the
/// marker or the data.
pub fn write_str_len<W: RmpWrite>(wr: &mut W, len: u32) -> Result<Marker, ValueWriteError<W::Error>> {
    if len < 32 {
        write_marker(wr, Marker::FixStr(len as u8))?;
        Ok(Marker::FixStr(len as u8))
    } else if len < 256 {
        write_marker(wr, Marker::Str8)?;
        wr.write_data_u8(len as u8)?;
        Ok(Marker::Str8)
    } else if len <= u16::MAX as u32 {
        write_marker(wr, Marker::Str16)?;
        wr.write_data_u16(len as u16)?;
        Ok(Marker::Str16)
    } else {
        write_marker(wr, Marker::Str32)?;
        wr.write_data_u32(len)?;
        Ok(Marker::Str32)
    }
}

/// Encodes and attempts to write the most efficient string binary representation to the
/// given `Write`.
///
/// # Errors
///
/// This function will return `ValueWriteError` on any I/O error occurred while writing either the
/// marker or the data.
///
/// [`ValueWriteError::OutOfRange`] if the string is longer than `u32::MAX` bytes, the longest MessagePack can encode.
pub fn write_str<W: RmpWrite>(wr: &mut W, data: &str) -> Result<(), ValueWriteError<W::Error>> {
    let len = u32::try_from(data.len()).map_err(|_| ValueWriteError::OutOfRange)?;
    write_str_len(wr, len)?;
    wr.write_bytes(data.as_bytes())
        .map_err(ValueWriteError::InvalidDataWrite)
}
