use bytes::BytesMut;
use chrono::{DateTime, NaiveDate, NaiveDateTime, NaiveTime, Utc};
use rust_decimal::Decimal;
use std::collections::HashMap;
use std::error::Error;
use tokio_postgres::Row;
use tokio_postgres::types::{to_sql_checked, IsNull, ToSql, Type};

use crate::udbc::value::Value;

#[derive(Debug)]
struct PgInt(i64);

impl PgInt {
    fn new(value: i64) -> Self {
        Self(value)
    }
}

impl ToSql for PgInt {
    fn to_sql(
        &self,
        ty: &Type,
        out: &mut BytesMut,
    ) -> Result<IsNull, Box<dyn Error + Sync + Send>> {
        match *ty {
            Type::INT2 => {
                let v = i16::try_from(self.0)?;
                v.to_sql(ty, out)
            }
            Type::INT4 => {
                let v = i32::try_from(self.0)?;
                v.to_sql(ty, out)
            }
            Type::INT8 => self.0.to_sql(ty, out),
            Type::TEXT | Type::VARCHAR | Type::BPCHAR | Type::NAME => {
                self.0.to_string().to_sql(ty, out)
            }
            _ => Err(Box::new(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                format!("Unsupported type: {:?}", ty),
            ))),
        }
    }

    fn accepts(ty: &Type) -> bool {
        matches!(
            *ty,
            Type::INT2 | Type::INT4 | Type::INT8 | Type::TEXT | Type::VARCHAR | Type::BPCHAR | Type::NAME
        )
    }

    to_sql_checked!();
}

#[derive(Debug)]
struct PgText(String);

impl PgText {
    fn new(value: String) -> Self {
        Self(value)
    }
}

impl ToSql for PgText {
    fn to_sql(
        &self,
        ty: &Type,
        out: &mut BytesMut,
    ) -> Result<IsNull, Box<dyn Error + Sync + Send>> {
        match *ty {
            Type::NUMERIC => {
                let v: Decimal = self.0.parse()?;
                v.to_sql(ty, out)
            }
            Type::DATE => {
                let v = NaiveDate::parse_from_str(&self.0, "%Y-%m-%d")?;
                v.to_sql(ty, out)
            }
            Type::TIME => {
                let v = NaiveTime::parse_from_str(&self.0, "%H:%M:%S%.f")?;
                v.to_sql(ty, out)
            }
            Type::TIMESTAMP => {
                let v = NaiveDateTime::parse_from_str(&self.0, "%Y-%m-%d %H:%M:%S%.f")?;
                v.to_sql(ty, out)
            }
            Type::TIMESTAMPTZ => {
                let v = DateTime::parse_from_rfc3339(&self.0)?.with_timezone(&Utc);
                v.to_sql(ty, out)
            }
            Type::TEXT | Type::VARCHAR | Type::BPCHAR | Type::NAME => self.0.to_sql(ty, out),
            _ => Err(Box::new(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                format!("Unsupported type: {:?}", ty),
            ))),
        }
    }

    fn accepts(ty: &Type) -> bool {
        matches!(
            *ty,
            Type::NUMERIC
                | Type::DATE
                | Type::TIME
                | Type::TIMESTAMP
                | Type::TIMESTAMPTZ
                | Type::TEXT
                | Type::VARCHAR
                | Type::BPCHAR
                | Type::NAME
        )
    }

    to_sql_checked!();
}

#[derive(Debug)]
struct PgNull;

impl ToSql for PgNull {
    fn to_sql(
        &self,
        _ty: &Type,
        _out: &mut BytesMut,
    ) -> Result<IsNull, Box<dyn Error + Sync + Send>> {
        Ok(IsNull::Yes)
    }

    fn accepts(_ty: &Type) -> bool {
        true
    }

    to_sql_checked!();
}

pub fn to_pg_param(value: &Value) -> Box<dyn ToSql + Sync + Send> {
    match value {
        Value::Null => Box::new(PgNull),
        Value::Bool(v) => Box::new(*v),
        Value::Char(c) => Box::new(c.to_string()),
        Value::Str(s) => Box::new(PgText::new(s.clone())),
        Value::I8(v) => Box::new(PgInt::new(*v as i64)),
        Value::I16(v) => Box::new(PgInt::new(*v as i64)),
        Value::I32(v) => Box::new(PgInt::new(*v as i64)),
        Value::I64(v) => Box::new(PgInt::new(*v)),
        Value::I128(v) => {
            if *v >= i64::MIN as i128 && *v <= i64::MAX as i128 {
                Box::new(PgInt::new(*v as i64))
            } else {
                Box::new(PgText::new(v.to_string()))
            }
        }
        Value::U8(v) => Box::new(PgInt::new(*v as i64)),
        Value::U16(v) => Box::new(PgInt::new(*v as i64)),
        Value::U32(v) => Box::new(PgInt::new(*v as i64)),
        Value::U64(v) => {
            if *v <= i64::MAX as u64 {
                Box::new(PgInt::new(*v as i64))
            } else {
                Box::new(PgText::new(v.to_string()))
            }
        }
        Value::U128(v) => {
            if *v <= i64::MAX as u128 {
                Box::new(PgInt::new(*v as i64))
            } else {
                Box::new(PgText::new(v.to_string()))
            }
        }
        Value::F32(v) => Box::new(*v),
        Value::F64(v) => Box::new(*v),
        Value::Bytes(v) => Box::new(v.clone()),
        Value::Date(v) => Box::new(*v),
        Value::Time(v) => Box::new(*v),
        Value::DateTime(v) => Box::new(*v),
        Value::DateTimeUtc(v) => Box::new(*v),
        Value::Decimal(v) => Box::new(v.clone()),
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
