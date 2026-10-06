pub use self::de::{EnumRefDeserializer, deserialize_from, from_value};
pub use self::se::to_value;
use crate::value::{IntPriv, Integer, Value, ValueRef};
use serde::de::Unexpected;
use std::error;
use std::fmt::{self, Display, Formatter};

mod de;
mod se;

#[derive(Debug)]
pub enum Error {
    Syntax(String),
}

impl Display for Error {
    #[cold]
    fn fmt(&self, fmt: &mut Formatter<'_>) -> Result<(), fmt::Error> {
        match *self {
            Error::Syntax(ref err) => write!(fmt, "error while decoding value: {}", err),
        }
    }
}

impl error::Error for Error {}

trait ValueExt {
    fn unexpected(&self) -> Unexpected<'_>;
}

impl ValueExt for Value {
    #[cold]
    fn unexpected(&self) -> Unexpected<'_> {
        match *self {
            Value::Nil => Unexpected::Unit,
            Value::Boolean(v) => Unexpected::Bool(v),
            Value::Integer(Integer { n }) => match n {
                IntPriv::PosInt(v) => Unexpected::Unsigned(v),
                IntPriv::NegInt(v) => Unexpected::Signed(v),
            },
            Value::F32(v) => Unexpected::Float(v as f64),
            Value::F64(v) => Unexpected::Float(v),
            Value::String(ref v) => match v.s {
                Ok(ref v) => Unexpected::Str(v),
                Err(ref v) => Unexpected::Bytes(&v.0[..]),
            },
            Value::Binary(ref v) => Unexpected::Bytes(v),
            Value::Array(..) => Unexpected::Seq,
            Value::Map(..) => Unexpected::Map,
            Value::Ext(..) => Unexpected::Seq,
        }
    }
}

impl ValueExt for ValueRef<'_> {
    #[cold]
    fn unexpected(&self) -> Unexpected<'_> {
        match *self {
            ValueRef::Nil => Unexpected::Unit,
            ValueRef::Boolean(v) => Unexpected::Bool(v),
            ValueRef::Integer(Integer { n }) => match n {
                IntPriv::PosInt(v) => Unexpected::Unsigned(v),
                IntPriv::NegInt(v) => Unexpected::Signed(v),
            },
            ValueRef::F32(v) => Unexpected::Float(v as f64),
            ValueRef::F64(v) => Unexpected::Float(v),
            ValueRef::String(ref v) => match v.s {
                Ok(v) => Unexpected::Str(v),
                Err(ref v) => Unexpected::Bytes(v.0),
            },
            ValueRef::Binary(v) => Unexpected::Bytes(v),
            ValueRef::Array(..) => Unexpected::Seq,
            ValueRef::Map(..) => Unexpected::Map,
            ValueRef::Ext(..) => Unexpected::Seq,
        }
    }
}
