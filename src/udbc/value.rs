use crate::error::DbError;
use crate::udbc;
use chrono::{DateTime, NaiveDate, NaiveDateTime, NaiveTime, Utc};
use rust_decimal::Decimal;
use serde::Serialize;
use std::collections::HashMap;

#[derive(Debug, Clone, PartialEq, Serialize)]
pub enum Value {
    Null,
    Bool(bool),
    I16(i16),
    I32(i32),
    I64(i64),
    U8(u8),
    F64(f64),
    Str(String),
    Bytes(Vec<u8>),
    Date(NaiveDate),
    Time(NaiveTime),
    DateTime(NaiveDateTime),
    DateTimeUtc(DateTime<Utc>),
    Decimal(Decimal),
    List(Vec<Value>),
    Map(HashMap<String, Value>),
}

/// 将 T: Serialize 转为 Vec<Value>
pub fn to_values<T: Serialize>(t: &T) -> Result<Vec<Value>, DbError> {
    let v = udbc::serializer::to_value(t);
    let out = match v {
        Value::List(vec) => vec,
        Value::Map(map) => map.into_values().collect(),
        other => vec![other],
    };
    Ok(out)
}

impl From<bool> for Value {
    fn from(v: bool) -> Self {
        Value::Bool(v)
    }
}
impl From<i16> for Value {
    fn from(v: i16) -> Self {
        Value::I16(v)
    }
}
impl From<i32> for Value {
    fn from(v: i32) -> Self {
        Value::I32(v)
    }
}
impl From<i64> for Value {
    fn from(v: i64) -> Self {
        Value::I64(v)
    }
}
impl From<u8> for Value {
    fn from(v: u8) -> Self {
        Value::U8(v)
    }
}
impl From<f64> for Value {
    fn from(v: f64) -> Self {
        Value::F64(v)
    }
}
impl From<String> for Value {
    fn from(v: String) -> Self {
        Value::Str(v)
    }
}
impl From<&str> for Value {
    fn from(v: &str) -> Self {
        Value::Str(v.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_to_values_unit() {
        let args = ();
        let values = to_values(&args).unwrap();
        assert_eq!(values.len(), 0);
    }

    #[test]
    fn test_to_values_tuple() {
        let args = (1, "hello");
        let values = to_values(&args).unwrap();
        assert_eq!(values.len(), 2);
        assert_eq!(values[0], Value::I32(1));
        assert_eq!(values[1], Value::Str("hello".to_string()));
    }
}
