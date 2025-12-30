use crate::udbc::value::Value;
use rusqlite::types::{Value as SqliteValue, ValueRef};

pub fn from_sqlite_value(v: ValueRef<'_>) -> Value {
    match v {
        ValueRef::Null => Value::Null,
        ValueRef::Integer(i) => Value::I64(i),
        ValueRef::Real(f) => Value::F64(f),
        ValueRef::Text(b) => match std::str::from_utf8(b) {
            Ok(s) => Value::Str(s.to_string()),
            Err(_) => Value::Bytes(b.to_vec()),
        },
        ValueRef::Blob(b) => Value::Bytes(b.to_vec()),
    }
}

pub fn to_sqlite_value(v: &Value) -> SqliteValue {
    match v {
        Value::Null => SqliteValue::Null,
        Value::Bool(b) => SqliteValue::Integer(if *b { 1 } else { 0 }),
        Value::I8(i) => SqliteValue::Integer(*i as i64),
        Value::I16(i) => SqliteValue::Integer(*i as i64),
        Value::I32(i) => SqliteValue::Integer(*i as i64),
        Value::I64(i) => SqliteValue::Integer(*i),
        Value::I128(i) => SqliteValue::Text(i.to_string()),
        Value::U8(u) => SqliteValue::Integer(*u as i64),
        Value::U16(u) => SqliteValue::Integer(*u as i64),
        Value::U32(u) => SqliteValue::Integer(*u as i64),
        Value::U64(u) => SqliteValue::Text(u.to_string()),
        Value::U128(u) => SqliteValue::Text(u.to_string()),
        Value::F32(f) => SqliteValue::Real(*f as f64),
        Value::F64(f) => SqliteValue::Real(*f),
        Value::Char(c) => SqliteValue::Text(c.to_string()),
        Value::Str(s) => SqliteValue::Text(s.clone()),
        Value::Bytes(b) => SqliteValue::Blob(b.clone()),
        Value::Date(d) => SqliteValue::Text(d.to_string()),
        Value::Time(t) => SqliteValue::Text(t.to_string()),
        Value::DateTime(dt) => SqliteValue::Text(dt.to_string()),
        Value::DateTimeUtc(dt) => SqliteValue::Text(dt.to_rfc3339()),
        Value::Decimal(d) => SqliteValue::Text(d.to_string()),
        Value::List(_) | Value::Map(_) => SqliteValue::Null,
    }
}
