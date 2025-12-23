use crate::error::DbError;
use crate::udbc::value::Value;
use serde::de::{self, Deserializer, IntoDeserializer, MapAccess, Visitor};
use std::collections::HashMap;

pub struct RowDeserializer<'a> {
    row: &'a HashMap<String, Value>,
}

impl<'a> RowDeserializer<'a> {
    pub fn new(row: &'a HashMap<String, Value>) -> Self {
        Self { row }
    }
}

impl<'de, 'a> Deserializer<'de> for RowDeserializer<'a> {
    type Error = DbError;

    fn deserialize_any<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        visitor.visit_map(RowMapAccess::new(self.row))
    }

    serde::forward_to_deserialize_any! {
        bool i8 i16 i32 i64 u8 u16 u32 u64 f32 f64 char str string
        unit seq tuple tuple_struct map struct enum identifier ignored_any
        unit_struct newtype_struct bytes byte_buf option
    }
}

struct RowMapAccess<'a> {
    iter: std::collections::hash_map::Iter<'a, String, Value>,
    current: Option<(&'a String, &'a Value)>,
}

impl<'a> RowMapAccess<'a> {
    fn new(row: &'a HashMap<String, Value>) -> Self {
        Self {
            iter: row.iter(),
            current: None,
        }
    }
}

impl<'de, 'a> MapAccess<'de> for RowMapAccess<'a> {
    type Error = DbError;

    fn next_key_seed<K>(&mut self, seed: K) -> Result<Option<K::Value>, Self::Error>
    where
        K: de::DeserializeSeed<'de>,
    {
        if let Some((k, v)) = self.iter.next() {
            self.current = Some((k, v));
            seed.deserialize(k.as_str().into_deserializer()).map(Some)
        } else {
            Ok(None)
        }
    }

    fn next_value_seed<V>(&mut self, seed: V) -> Result<V::Value, Self::Error>
    where
        V: de::DeserializeSeed<'de>,
    {
        let (_k, v) = self.current.take().unwrap();
        seed.deserialize(ValueDeserializer { value: v })
    }
}

struct ValueSeqAccess<'a> {
    iter: std::slice::Iter<'a, Value>,
}

impl<'a> ValueSeqAccess<'a> {
    fn new(list: &'a [Value]) -> Self {
        Self { iter: list.iter() }
    }
}

impl<'de, 'a> de::SeqAccess<'de> for ValueSeqAccess<'a> {
    type Error = DbError;

    fn next_element_seed<T>(&mut self, seed: T) -> Result<Option<T::Value>, Self::Error>
    where
        T: de::DeserializeSeed<'de>,
    {
        if let Some(v) = self.iter.next() {
            seed.deserialize(ValueDeserializer { value: v }).map(Some)
        } else {
            Ok(None)
        }
    }
}

pub struct ValueDeserializer<'a> {
    pub value: &'a Value,
}

impl<'de, 'a> Deserializer<'de> for ValueDeserializer<'a> {
    type Error = DbError;

    fn deserialize_any<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        match self.value {
            Value::Null => visitor.visit_unit(),
            Value::Bool(v) => visitor.visit_bool(*v),
            Value::I16(v) => visitor.visit_i16(*v),
            Value::I32(v) => visitor.visit_i32(*v),
            Value::I64(v) => visitor.visit_i64(*v),
            Value::U8(v) => visitor.visit_u8(*v),
            Value::F64(v) => visitor.visit_f64(*v),
            Value::Str(v) => visitor.visit_str(v),
            Value::Bytes(v) => visitor.visit_bytes(v),
            Value::Date(d) => visitor.visit_string(d.to_string()),
            Value::Time(t) => visitor.visit_string(t.to_string()),
            Value::DateTime(dt) => visitor.visit_string(dt.to_string()),
            Value::DateTimeUtc(dt) => visitor.visit_string(dt.to_rfc3339()),
            Value::Decimal(d) => visitor.visit_string(d.to_string()),
            Value::List(list) => visitor.visit_seq(ValueSeqAccess::new(list)),
            Value::Map(map) => visitor.visit_map(RowMapAccess::new(map)),
        }
    }

    fn deserialize_map<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        match self.value {
            Value::List(list) if list.len() == 1 => {
                // If expecting a map but got a list of 1, try to deserialize the single element
                let inner = &list[0];
                ValueDeserializer { value: inner }.deserialize_map(visitor)
            }
            _ => self.deserialize_any(visitor),
        }
    }

    fn deserialize_struct<V>(
        self,
        name: &'static str,
        fields: &'static [&'static str],
        visitor: V,
    ) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        match self.value {
            Value::List(list) if list.len() == 1 => {
                // If expecting a struct but got a list of 1, try to deserialize the single element
                let inner = &list[0];
                ValueDeserializer { value: inner }.deserialize_struct(name, fields, visitor)
            }
            _ => self.deserialize_any(visitor),
        }
    }

    fn deserialize_ignored_any<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        visitor.visit_unit()
    }

    fn deserialize_option<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        match self.value {
            Value::Null => visitor.visit_none(),
            _ => visitor.visit_some(self),
        }
    }

    serde::forward_to_deserialize_any! {
        bool i8 i16 i32 i64 u8 u16 u32 u64 f32 f64 char str string
        unit seq tuple tuple_struct enum identifier
        unit_struct newtype_struct bytes byte_buf
    }
}
