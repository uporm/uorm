use chrono::{DateTime, NaiveDate, NaiveDateTime, NaiveTime, Utc};
use rust_decimal::Decimal;
use std::collections::HashMap;
use crate::error::DbError;

#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    Null,
    Bool(bool),
    I8(i8),
    I16(i16),
    I32(i32),
    I64(i64),
    I128(i128),
    U8(u8),
    U16(u16),
    U32(u32),
    U64(u64),
    U128(u128),
    F32(f32),
    F64(f64),
    Char(char),
    Str(String),
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

/// 任何能转换为 Value 的类型
pub trait ToValue {
    fn to_value(&self) -> Value;
}

/// 任何能从 Value 还原的类型
pub trait FromValue: Sized {
    fn from_value(v: Value) -> Result<Self, DbError>;
}

// --- 基础类型的宏实现 ---

macro_rules! impl_primitive {
    ($rust_type:ty, $variant:ident) => {
        impl ToValue for $rust_type {
            fn to_value(&self) -> Value {
                Value::$variant(self.clone())
            }
        }
    };
}

macro_rules! impl_from_value_int {
    ($rust_type:ty) => {
        impl FromValue for $rust_type {
            fn from_value(v: Value) -> Result<Self, DbError> {
                match v {
                    Value::I8(n) => <$rust_type>::try_from(n).map_err(|_| DbError::TypeMismatch(format!("Value {} out of range for {}", n, stringify!($rust_type)))),
                    Value::I16(n) => <$rust_type>::try_from(n).map_err(|_| DbError::TypeMismatch(format!("Value {} out of range for {}", n, stringify!($rust_type)))),
                    Value::I32(n) => <$rust_type>::try_from(n).map_err(|_| DbError::TypeMismatch(format!("Value {} out of range for {}", n, stringify!($rust_type)))),
                    Value::I64(n) => <$rust_type>::try_from(n).map_err(|_| DbError::TypeMismatch(format!("Value {} out of range for {}", n, stringify!($rust_type)))),
                    Value::I128(n) => <$rust_type>::try_from(n).map_err(|_| DbError::TypeMismatch(format!("Value {} out of range for {}", n, stringify!($rust_type)))),
                    Value::U8(n) => <$rust_type>::try_from(n).map_err(|_| DbError::TypeMismatch(format!("Value {} out of range for {}", n, stringify!($rust_type)))),
                    Value::U16(n) => <$rust_type>::try_from(n).map_err(|_| DbError::TypeMismatch(format!("Value {} out of range for {}", n, stringify!($rust_type)))),
                    Value::U32(n) => <$rust_type>::try_from(n).map_err(|_| DbError::TypeMismatch(format!("Value {} out of range for {}", n, stringify!($rust_type)))),
                    Value::U64(n) => <$rust_type>::try_from(n).map_err(|_| DbError::TypeMismatch(format!("Value {} out of range for {}", n, stringify!($rust_type)))),
                    Value::U128(n) => <$rust_type>::try_from(n).map_err(|_| DbError::TypeMismatch(format!("Value {} out of range for {}", n, stringify!($rust_type)))),
                    _ => Err(DbError::TypeMismatch(format!("Expected numeric value, got {:?}", v))),
                }
            }
        }
    };
}

// 批量实现基础类型
impl_primitive!(i8, I8);
impl_primitive!(i16, I16);
impl_primitive!(i32, I32);
impl_primitive!(i64, I64);
impl_primitive!(i128, I128);
impl_primitive!(u8, U8);
impl_primitive!(u16, U16);
impl_primitive!(u32, U32);
impl_primitive!(u64, U64);
impl_primitive!(u128, U128);

impl_from_value_int!(i8);
impl_from_value_int!(i16);
impl_from_value_int!(i32);
impl_from_value_int!(i64);
impl_from_value_int!(i128);
impl_from_value_int!(u8);
impl_from_value_int!(u16);
impl_from_value_int!(u32);
impl_from_value_int!(u64);
impl_from_value_int!(u128);

impl_primitive!(f32, F32);
impl FromValue for f32 {
    fn from_value(v: Value) -> Result<Self, DbError> {
        if let Value::F32(val) = v {
            Ok(val)
        } else if let Value::F64(val) = v {
             Ok(val as f32)
        } else {
            Err(DbError::TypeMismatch(format!(
                "Expected F32, got {:?}", v
            )))
        }
    }
}

impl_primitive!(f64, F64);
impl FromValue for f64 {
    fn from_value(v: Value) -> Result<Self, DbError> {
         if let Value::F64(val) = v {
            Ok(val)
        } else if let Value::F32(val) = v {
             Ok(val as f64)
        } else {
            Err(DbError::TypeMismatch(format!(
                "Expected F64, got {:?}", v
            )))
        }
    }
}

impl_primitive!(char, Char);
impl FromValue for char {
    fn from_value(v: Value) -> Result<Self, DbError> {
        if let Value::Char(val) = v {
            Ok(val)
        } else {
            Err(DbError::TypeMismatch(format!(
                "Expected Char, got {:?}", v
            )))
        }
    }
}
// impl_primitive!(String, Str); // Removed default String implementation

impl ToValue for String {
    fn to_value(&self) -> Value {
        Value::Str(self.clone())
    }
}

impl FromValue for String {
    fn from_value(v: Value) -> Result<Self, DbError> {
        match v {
            Value::Str(s) => Ok(s),
            Value::Bytes(b) => String::from_utf8(b)
                .map_err(|e| DbError::TypeMismatch(format!("Invalid UTF-8 bytes: {}", e))),
            _ => Err(DbError::TypeMismatch(format!(
                "Expected Str or Bytes, got {:?}",
                v
            ))),
        }
    }
}
impl ToValue for &str {
    fn to_value(&self) -> Value {
        Value::Str(self.to_string())
    }
}

// Allow Value to be passed as argument
impl ToValue for Value {
    fn to_value(&self) -> Value {
        self.clone()
    }
}

impl ToValue for bool {
    fn to_value(&self) -> Value {
        Value::Bool(*self)
    }
}

impl FromValue for bool {
    fn from_value(v: Value) -> Result<Self, DbError> {
        match v {
            Value::Bool(b) => Ok(b),
            // Some DBs return 0/1 for bool
            Value::I8(n) => Ok(n != 0),
            Value::I16(n) => Ok(n != 0),
            Value::I32(n) => Ok(n != 0),
            Value::I64(n) => Ok(n != 0),
            _ => Err(DbError::TypeMismatch(format!(
                "Expected Bool or number, got {:?}",
                v
            ))),
        }
    }
}

// Allow Value to be returned as result
impl FromValue for Value {
    fn from_value(v: Value) -> Result<Self, DbError> {
        Ok(v)
    }
}

// Implement FromValue for unit type () to allow functions returning Result<()>
impl FromValue for () {
    fn from_value(_v: Value) -> Result<Self, DbError> {
        Ok(())
    }
}

impl ToValue for () {
    fn to_value(&self) -> Value {
        Value::Null
    }
}

// Blanket implementation for references
impl<'a, T> ToValue for &'a T
where
    T: ToValue,
{
    fn to_value(&self) -> Value {
        (**self).to_value()
    }
}

// === 泛型容器实现 ===

// Option
impl<T: ToValue> ToValue for Option<T> {
    fn to_value(&self) -> Value {
        match self {
            Some(v) => v.to_value(),
            None => Value::Null,
        }
    }
}
impl<T: FromValue> FromValue for Option<T> {
    fn from_value(v: Value) -> Result<Self, DbError> {
        match v {
            Value::Null => Ok(None),
            _ => Ok(Some(T::from_value(v)?)),
        }
    }
}

// Vec
impl<T: ToValue> ToValue for Vec<T> {
    fn to_value(&self) -> Value {
        Value::List(self.iter().map(|v| v.to_value()).collect())
    }
}
impl<T: FromValue> FromValue for Vec<T> {
    fn from_value(v: Value) -> Result<Self, DbError> {
        match v {
            Value::List(l) => l.into_iter().map(T::from_value).collect(),
            _ => Err(DbError::TypeMismatch(format!("Expected List, got {:?}", v))),
        }
    }
}

// HashMap
impl<T: ToValue> ToValue for HashMap<String, T> {
    fn to_value(&self) -> Value {
        let mut map = HashMap::new();
        for (k, v) in self {
            map.insert(k.clone(), v.to_value());
        }
        Value::Map(map)
    }
}
impl<T: FromValue> FromValue for HashMap<String, T> {
    fn from_value(v: Value) -> Result<Self, DbError> {
        match v {
            Value::Map(m) => {
                let mut out = HashMap::new();
                for (k, val) in m {
                    out.insert(k, T::from_value(val)?);
                }
                Ok(out)
            }
            _ => Err(DbError::TypeMismatch(format!("Expected Map, got {:?}", v))),
        }
    }
}