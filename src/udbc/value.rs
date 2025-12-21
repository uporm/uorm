use chrono::{DateTime, NaiveDate, NaiveDateTime, NaiveTime, Utc};
use rust_decimal::Decimal;
use std::collections::HashMap;

/// Generic value type used to represent database-compatible values.
///
/// This enum is designed to:
/// - Act as an intermediate representation between Rust types and database values
/// - Be serializable via `serde`
/// - Support common scalar, temporal, numeric, and composite types
#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    /// Represents SQL NULL or absence of value
    Null,

    /// Boolean value
    Bool(bool),

    /// Signed 16-bit integer
    I16(i16),

    /// Signed 32-bit integer
    I32(i32),

    /// Signed 64-bit integer
    I64(i64),

    /// Unsigned 8-bit integer
    U8(u8),

    /// 64-bit floating point number
    F64(f64),

    /// UTF-8 string
    Str(String),

    /// Raw binary data
    Bytes(Vec<u8>),

    /// Date without time zone
    Date(NaiveDate),

    /// Time without date
    Time(NaiveTime),

    /// Date and time without time zone
    DateTime(NaiveDateTime),

    /// Date and time in UTC
    DateTimeUtc(DateTime<Utc>),

    /// Arbitrary-precision decimal number
    Decimal(Decimal),

    /// Ordered list of values (e.g. arrays, tuples)
    List(Vec<Value>),

    /// Key-value map (e.g. structs, JSON objects)
    Map(HashMap<String, Value>),
}

/* -------------------------------------------------------------------------- */
/*                          From<T> implementations                           */
/* -------------------------------------------------------------------------- */

/// Allow automatic conversion from `bool` into `Value`
impl From<bool> for Value {
    fn from(v: bool) -> Self {
        Value::Bool(v)
    }
}

/// Allow automatic conversion from `i16` into `Value`
impl From<i16> for Value {
    fn from(v: i16) -> Self {
        Value::I16(v)
    }
}

/// Allow automatic conversion from `i32` into `Value`
impl From<i32> for Value {
    fn from(v: i32) -> Self {
        Value::I32(v)
    }
}

/// Allow automatic conversion from `i64` into `Value`
impl From<i64> for Value {
    fn from(v: i64) -> Self {
        Value::I64(v)
    }
}

/// Allow automatic conversion from `u8` into `Value`
impl From<u8> for Value {
    fn from(v: u8) -> Self {
        Value::U8(v)
    }
}

/// Allow automatic conversion from `f64` into `Value`
impl From<f64> for Value {
    fn from(v: f64) -> Self {
        Value::F64(v)
    }
}

/// Allow automatic conversion from `String` into `Value`
impl From<String> for Value {
    fn from(v: String) -> Self {
        Value::Str(v)
    }
}

/// Allow automatic conversion from `&str` into `Value`
impl From<&str> for Value {
    fn from(v: &str) -> Self {
        Value::Str(v.to_string())
    }
}
