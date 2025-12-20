use crate::udbc::value::Value;
use chrono::{Datelike, NaiveDate, NaiveDateTime, NaiveTime, Timelike};
use mysql_async::Value as MyValue;

pub fn from_mysql_value(v: &MyValue) -> Value {
    match v {
        MyValue::NULL => Value::Null,
        MyValue::Int(i) => Value::I64(*i),
        MyValue::UInt(u) => Value::I64(*u as i64),
        MyValue::Float(f) => Value::F64(*f as f64),
        MyValue::Double(d) => Value::F64(*d),
        MyValue::Bytes(b) => Value::Bytes(b.clone()),
        MyValue::Date(y, m, d, h, min, s, micro) => {
            if *h == 0 && *min == 0 && *s == 0 && *micro == 0 {
                Value::Date(NaiveDate::from_ymd_opt(*y as i32, *m as u32, *d as u32).unwrap())
            } else {
                let dt = NaiveDate::from_ymd_opt(*y as i32, *m as u32, *d as u32)
                    .unwrap()
                    .and_hms_micro_opt(*h as u32, *min as u32, *s as u32, *micro as u32)
                    .unwrap();
                Value::DateTime(dt)
            }
        }
        MyValue::Time(is_neg, days, h, min, s, micro) => {
            let total_h = *days * 24 + (*h as u32);
            let t = NaiveTime::from_hms_micro_opt(total_h, *min as u32, *s as u32, *micro as u32)
                .unwrap();
            if *is_neg {
                Value::Str(format!("-{}", t))
            } else {
                Value::Time(t)
            }
        }
    }
}

pub fn to_mysql_value(v: &Value) -> MyValue {
    match v {
        Value::Null => MyValue::NULL,
        Value::Bool(b) => MyValue::Int(if *b { 1 } else { 0 }),
        Value::I16(i) => MyValue::Int(*i as i64),
        Value::I32(i) => MyValue::Int(*i as i64),
        Value::I64(i) => MyValue::Int(*i),
        Value::U8(u) => MyValue::UInt(*u as u64),
        Value::F64(f) => MyValue::Double(*f),
        Value::Str(s) => MyValue::Bytes(s.clone().into_bytes()),
        Value::Bytes(b) => MyValue::Bytes(b.clone()),
        Value::Date(d) => MyValue::Date(
            d.year() as u16,
            d.month() as u8,
            d.day() as u8,
            0u8,
            0u8,
            0u8,
            0u32,
        ),
        Value::Time(t) => MyValue::Time(
            false,
            0u32,
            t.hour() as u8,
            t.minute() as u8,
            t.second() as u8,
            t.nanosecond() / 1000,
        ),
        Value::DateTime(dt) => MyValue::Date(
            dt.date().year() as u16,
            dt.date().month() as u8,
            dt.date().day() as u8,
            dt.time().hour() as u8,
            dt.time().minute() as u8,
            dt.time().second() as u8,
            dt.and_utc().timestamp_subsec_micros(),
        ),
        Value::DateTimeUtc(dt) => {
            let ndt: NaiveDateTime = dt.naive_utc();
            MyValue::Date(
                ndt.date().year() as u16,
                ndt.date().month() as u8,
                ndt.date().day() as u8,
                ndt.time().hour() as u8,
                ndt.time().minute() as u8,
                ndt.time().second() as u8,
                ndt.and_utc().timestamp_subsec_micros(),
            )
        }
        Value::Decimal(d) => MyValue::Bytes(d.to_string().into_bytes()),
        Value::List(_) | Value::Map(_) => MyValue::Bytes(Vec::new()),
    }
}
