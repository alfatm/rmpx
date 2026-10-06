use super::Error;
use crate::raw::encode::{
    write_array_len, write_bin, write_bool, write_ext_meta, write_f32, write_f64, write_map_len, write_nil, write_sint,
    write_str, write_uint,
};
use crate::value::{IntPriv, Integer, Utf8String, Value};
use std::io::Write;

/// Encodes and attempts to write the most efficient representation of the given Value.
///
/// # Note
///
/// All instances of `ErrorKind::Interrupted` are handled by this function and the underlying
/// operation is retried.
pub fn write_value<W>(wr: &mut W, val: &Value) -> Result<(), Error>
where
    W: Write,
{
    match *val {
        Value::Nil => {
            write_nil(wr).map_err(Error::InvalidMarkerWrite)?;
        }
        Value::Boolean(val) => {
            write_bool(wr, val).map_err(Error::InvalidMarkerWrite)?;
        }
        Value::Integer(Integer { n }) => match n {
            IntPriv::PosInt(n) => {
                write_uint(wr, n)?;
            }
            IntPriv::NegInt(n) => {
                write_sint(wr, n)?;
            }
        },
        Value::F32(val) => {
            write_f32(wr, val)?;
        }
        Value::F64(val) => {
            write_f64(wr, val)?;
        }
        Value::String(Utf8String { ref s }) => match *s {
            Ok(ref val) => {
                super::len_u32(val.len())?;
                write_str(wr, val)?
            }
            Err(ref err) => {
                super::len_u32(err.0.len())?;
                write_bin(wr, &err.0)?
            }
        },
        Value::Binary(ref val) => {
            super::len_u32(val.len())?;
            write_bin(wr, val)?;
        }
        Value::Array(ref vec) => {
            write_array_len(wr, super::len_u32(vec.len())?)?;
            for v in vec {
                write_value(wr, v)?;
            }
        }
        Value::Map(ref map) => {
            write_map_len(wr, super::len_u32(map.len())?)?;
            for (key, val) in map {
                write_value(wr, key)?;
                write_value(wr, val)?;
            }
        }
        Value::Ext(ty, ref data) => {
            write_ext_meta(wr, super::len_u32(data.len())?, ty)?;
            wr.write_all(data).map_err(Error::InvalidDataWrite)?;
        }
    }

    Ok(())
}
