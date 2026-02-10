use chrono::{DateTime, NaiveDate, NaiveDateTime, NaiveTime, Utc};
use std::collections::HashMap;
use tokio_postgres::Row;
use tokio_postgres::types::{ToSql, Type};

use crate::udbc::value::Value;

pub fn to_pg_param(value: &Value) -> Box<dyn ToSql + Sync + Send> {
    match value {
        Value::Null => Box::new(None::<i32>),
        Value::Bool(v) => Box::new(*v),
        Value::Char(c) => Box::new(c.to_string()),
        Value::Str(s) => Box::new(s.clone()),
        Value::I8(v) => Box::new(*v as i16),
        Value::I16(v) => Box::new(*v),
        Value::I32(v) => Box::new(*v),
        Value::I64(v) => Box::new(*v),
        Value::I128(v) => {
            if *v >= i64::MIN as i128 && *v <= i64::MAX as i128 {
                Box::new(*v as i64)
            } else {
                Box::new(v.to_string())
            }
        }
        Value::U8(v) => Box::new(*v as i16),
        Value::U16(v) => Box::new(*v as i32),
        Value::U32(v) => Box::new(*v as i64),
        Value::U64(v) => {
            if *v <= i64::MAX as u64 {
                Box::new(*v as i64)
            } else {
                Box::new(v.to_string())
            }
        }
        Value::U128(v) => {
            if *v <= i64::MAX as u128 {
                Box::new(*v as i64)
            } else {
                Box::new(v.to_string())
            }
        }
        Value::F32(v) => Box::new(*v),
        Value::F64(v) => Box::new(*v),
        Value::Bytes(v) => Box::new(v.clone()),
        Value::Date(v) => Box::new(*v),
        Value::Time(v) => Box::new(*v),
        Value::DateTime(v) => Box::new(*v),
        Value::DateTimeUtc(v) => Box::new(*v),
        Value::Decimal(v) => Box::new(v.to_string()),
        Value::List(v) => Box::new(format!("{:?}", v)),
        Value::Map(v) => Box::new(format!("{:?}", v)),
    }
}

pub fn from_pg_row(row: &Row) -> HashMap<String, Value> {
    let mut out = HashMap::with_capacity(row.len());
    for (idx, col) in row.columns().iter().enumerate() {
        let v = match *col.type_() {
            Type::BOOL => option_value(row.try_get::<_, Option<bool>>(idx), Value::Bool),
            Type::INT2 => option_value(row.try_get::<_, Option<i16>>(idx), Value::I16),
            Type::INT4 => option_value(row.try_get::<_, Option<i32>>(idx), Value::I32),
            Type::INT8 => option_value(row.try_get::<_, Option<i64>>(idx), Value::I64),
            Type::FLOAT4 => option_value(row.try_get::<_, Option<f32>>(idx), Value::F32),
            Type::FLOAT8 => option_value(row.try_get::<_, Option<f64>>(idx), Value::F64),
            Type::NUMERIC => option_value(row.try_get::<_, Option<String>>(idx), Value::Str),
            Type::TEXT | Type::VARCHAR | Type::BPCHAR | Type::NAME => {
                option_value(row.try_get::<_, Option<String>>(idx), Value::Str)
            }
            Type::BYTEA => option_value(row.try_get::<_, Option<Vec<u8>>>(idx), Value::Bytes),
            Type::DATE => option_value(row.try_get::<_, Option<NaiveDate>>(idx), Value::Date),
            Type::TIME => option_value(row.try_get::<_, Option<NaiveTime>>(idx), Value::Time),
            Type::TIMESTAMP => {
                option_value(row.try_get::<_, Option<NaiveDateTime>>(idx), Value::DateTime)
            }
            Type::TIMESTAMPTZ => {
                option_value(row.try_get::<_, Option<DateTime<Utc>>>(idx), Value::DateTimeUtc)
            }
            _ => fallback_value(row, idx),
        };
        out.insert(col.name().to_string(), v);
    }
    out
}

fn option_value<T, F>(value: Result<Option<T>, tokio_postgres::Error>, map: F) -> Value
where
    F: FnOnce(T) -> Value,
{
    match value {
        Ok(Some(v)) => map(v),
        Ok(None) => Value::Null,
        Err(_) => Value::Null,
    }
}

fn fallback_value(row: &Row, idx: usize) -> Value {
    if let Ok(Some(v)) = row.try_get::<_, Option<String>>(idx) {
        return Value::Str(v);
    }
    if let Ok(Some(v)) = row.try_get::<_, Option<Vec<u8>>>(idx) {
        return Value::Bytes(v);
    }
    Value::Null
}
