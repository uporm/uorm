use crate::error::DbError;
use crate::udbc::value::Value;
use serde::Serialize;
use serde::ser::*;

use std::collections::HashMap;

pub struct ValueSerializer;

impl Serializer for ValueSerializer {
    type Ok = Value;
    type Error = DbError;
    type SerializeSeq = ListSerializer;
    type SerializeTuple = ListSerializer;
    type SerializeTupleStruct = ListSerializer;
    type SerializeTupleVariant = ListSerializer;
    type SerializeMap = MapSerializer;
    type SerializeStruct = MapSerializer;
    type SerializeStructVariant = MapSerializer;

    fn serialize_bool(self, v: bool) -> Result<Self::Ok, Self::Error> {
        Ok(Value::Bool(v))
    }
    fn serialize_i8(self, v: i8) -> Result<Self::Ok, Self::Error> {
        Ok(Value::I16(v as i16))
    }
    fn serialize_i16(self, v: i16) -> Result<Self::Ok, Self::Error> {
        Ok(Value::I16(v))
    }
    fn serialize_i32(self, v: i32) -> Result<Self::Ok, Self::Error> {
        Ok(Value::I32(v))
    }
    fn serialize_i64(self, v: i64) -> Result<Self::Ok, Self::Error> {
        Ok(Value::I64(v))
    }
    fn serialize_u8(self, v: u8) -> Result<Self::Ok, Self::Error> {
        Ok(Value::U8(v))
    }
    fn serialize_u16(self, v: u16) -> Result<Self::Ok, Self::Error> {
        Ok(Value::I64(v as i64))
    }
    fn serialize_u32(self, v: u32) -> Result<Self::Ok, Self::Error> {
        Ok(Value::I64(v as i64))
    }
    fn serialize_u64(self, v: u64) -> Result<Self::Ok, Self::Error> {
        Ok(Value::I64(v as i64))
    }
    fn serialize_f32(self, v: f32) -> Result<Self::Ok, Self::Error> {
        Ok(Value::F64(v as f64))
    }
    fn serialize_f64(self, v: f64) -> Result<Self::Ok, Self::Error> {
        Ok(Value::F64(v))
    }
    fn serialize_char(self, v: char) -> Result<Self::Ok, Self::Error> {
        Ok(Value::Str(v.to_string()))
    }
    fn serialize_str(self, v: &str) -> Result<Self::Ok, Self::Error> {
        Ok(Value::Str(v.to_string()))
    }
    fn serialize_bytes(self, v: &[u8]) -> Result<Self::Ok, Self::Error> {
        Ok(Value::Bytes(v.to_vec()))
    }
    fn serialize_none(self) -> Result<Self::Ok, Self::Error> {
        Ok(Value::Null)
    }
    fn serialize_some<T: ?Sized + Serialize>(self, value: &T) -> Result<Self::Ok, Self::Error> {
        value.serialize(self)
    }
    fn serialize_unit(self) -> Result<Self::Ok, Self::Error> {
        Ok(Value::List(vec![]))
    }
    fn serialize_unit_struct(self, _: &'static str) -> Result<Self::Ok, Self::Error> {
        Ok(Value::Null)
    }
    fn serialize_unit_variant(
        self,
        _: &'static str,
        _: u32,
        variant: &'static str,
    ) -> Result<Self::Ok, Self::Error> {
        Ok(Value::Str(variant.to_string()))
    }
    fn serialize_newtype_struct<T: ?Sized + Serialize>(
        self,
        _: &'static str,
        value: &T,
    ) -> Result<Self::Ok, Self::Error> {
        value.serialize(self)
    }
    fn serialize_newtype_variant<T: ?Sized + Serialize>(
        self,
        _: &'static str,
        _: u32,
        _: &'static str,
        value: &T,
    ) -> Result<Self::Ok, Self::Error> {
        value.serialize(self)
    }
    fn serialize_seq(self, len: Option<usize>) -> Result<Self::SerializeSeq, Self::Error> {
        Ok(ListSerializer {
            vec: Vec::with_capacity(len.unwrap_or(0)),
        })
    }
    fn serialize_tuple(self, len: usize) -> Result<Self::SerializeTuple, Self::Error> {
        self.serialize_seq(Some(len))
    }
    fn serialize_tuple_struct(
        self,
        _: &'static str,
        len: usize,
    ) -> Result<Self::SerializeTupleStruct, Self::Error> {
        self.serialize_seq(Some(len))
    }
    fn serialize_tuple_variant(
        self,
        _: &'static str,
        _: u32,
        _: &'static str,
        _: usize,
    ) -> Result<Self::SerializeTupleVariant, Self::Error> {
        self.serialize_seq(None)
    }
    fn serialize_map(self, len: Option<usize>) -> Result<Self::SerializeMap, Self::Error> {
        Ok(MapSerializer {
            map: HashMap::with_capacity(len.unwrap_or(0)),
            key: None,
        })
    }
    fn serialize_struct(
        self,
        _: &'static str,
        len: usize,
    ) -> Result<Self::SerializeStruct, Self::Error> {
        Ok(MapSerializer {
            map: HashMap::with_capacity(len),
            key: None,
        })
    }
    fn serialize_struct_variant(
        self,
        _: &'static str,
        _: u32,
        _: &'static str,
        len: usize,
    ) -> Result<Self::SerializeStructVariant, Self::Error> {
        Ok(MapSerializer {
            map: HashMap::with_capacity(len),
            key: None,
        })
    }
}

pub struct ListSerializer {
    vec: Vec<Value>,
}

macro_rules! impl_serialize_seq {
    ($trait:ident, $method:ident) => {
        impl $trait for ListSerializer {
            type Ok = Value;
            type Error = DbError;

            fn $method<T: ?Sized + Serialize>(&mut self, value: &T) -> Result<(), Self::Error> {
                self.vec.push(value.serialize(ValueSerializer)?);
                Ok(())
            }

            fn end(self) -> Result<Self::Ok, Self::Error> {
                Ok(Value::List(self.vec))
            }
        }
    };
}

impl_serialize_seq!(SerializeSeq, serialize_element);
impl_serialize_seq!(SerializeTuple, serialize_element);
impl_serialize_seq!(SerializeTupleStruct, serialize_field);
impl_serialize_seq!(SerializeTupleVariant, serialize_field);

pub struct MapSerializer {
    pub map: HashMap<String, Value>,
    pub key: Option<String>,
}

impl SerializeMap for MapSerializer {
    type Ok = Value;
    type Error = DbError;

    fn serialize_key<T: ?Sized + Serialize>(&mut self, key: &T) -> Result<(), Self::Error> {
        let k = key.serialize(ValueSerializer)?;
        if let Value::Str(s) = k {
            self.key = Some(s);
            Ok(())
        } else {
            Err(DbError::Value("Map key must be string".to_string()))
        }
    }

    fn serialize_value<T: ?Sized + Serialize>(&mut self, value: &T) -> Result<(), Self::Error> {
        let v = value.serialize(ValueSerializer)?;
        let key = self
            .key
            .take()
            .ok_or_else(|| DbError::Value("Missing key for value".to_string()))?;
        self.map.insert(key, v);
        Ok(())
    }

    fn end(self) -> Result<Self::Ok, Self::Error> {
        Ok(Value::Map(self.map))
    }
}

macro_rules! impl_serialize_struct {
    ($trait:ident) => {
        impl $trait for MapSerializer {
            type Ok = Value;
            type Error = DbError;

            fn serialize_field<T: ?Sized + Serialize>(
                &mut self,
                key: &'static str,
                value: &T,
            ) -> Result<(), Self::Error> {
                let v = value.serialize(ValueSerializer)?;
                self.map.insert(key.to_string(), v);
                Ok(())
            }

            fn end(self) -> Result<Self::Ok, Self::Error> {
                Ok(Value::Map(self.map))
            }
        }
    };
}

impl_serialize_struct!(SerializeStruct);
impl_serialize_struct!(SerializeStructVariant);

/* -------------------------------------------------------------------------- */
/*                                   Tests                                    */
/* -------------------------------------------------------------------------- */

#[cfg(test)]
mod tests {
    use crate::udbc::serializer::ValueSerializer;
    use crate::udbc::value::Value;
    use serde::Serialize;

    /// Unit type should produce an empty parameter list
    #[test]
    fn test_to_values_unit() {
        let args = ();
        let values = args.serialize(ValueSerializer).unwrap();
        if let Value::List(list) = values {
            assert_eq!(list.len(), 0);
        } else {
            panic!("Expected Value::List, got {:?}", values);
        }
    }

    /// Tuple should be converted into multiple values in order
    #[test]
    fn test_to_values_tuple() {
        let args = (1, "hello");
        let values = args.serialize(ValueSerializer).unwrap();
        if let Value::List(list) = values {
            assert_eq!(list.len(), 2);
            assert_eq!(list[0], Value::I32(1));
            assert_eq!(list[1], Value::Str("hello".to_string()));
        } else {
            panic!("Expected Value::List, got {:?}", values);
        }
    }
}
