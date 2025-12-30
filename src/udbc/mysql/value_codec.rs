use crate::udbc::value::Value;
use chrono::{Datelike, NaiveDate, NaiveTime, Timelike};
use mysql_async::Value as MyValue;

pub fn from_mysql_value(v: MyValue) -> Value {
    match v {
        MyValue::NULL => Value::Null,
        MyValue::Int(i) => Value::I64(i),
        MyValue::UInt(u) => Value::I64(u as i64),
        MyValue::Float(f) => Value::F64(f as f64),
        MyValue::Double(d) => Value::F64(d),
        MyValue::Bytes(b) => Value::Bytes(b),
        MyValue::Date(y, m, d, h, min, s, micro) => {
            let date = NaiveDate::from_ymd_opt(y as i32, m as u32, d as u32).unwrap_or_default();
            if h == 0 && min == 0 && s == 0 && micro == 0 {
                Value::Date(date)
            } else {
                let dt = date
                    .and_hms_micro_opt(h as u32, min as u32, s as u32, micro)
                    .unwrap_or_default();
                Value::DateTime(dt)
            }
        }
        MyValue::Time(is_neg, days, h, min, s, micro) => {
            let total_h = days * 24 + (h as u32);
            let t = NaiveTime::from_hms_micro_opt(total_h, min as u32, s as u32, micro)
                .unwrap_or_default();
            if is_neg {
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
        Value::I8(i) => MyValue::Int(*i as i64),
        Value::I16(i) => MyValue::Int(*i as i64),
        Value::I32(i) => MyValue::Int(*i as i64),
        Value::I64(i) => MyValue::Int(*i),
        Value::I128(i) => MyValue::Bytes(i.to_string().into_bytes()),
        Value::U8(u) => MyValue::UInt(*u as u64),
        Value::U16(u) => MyValue::UInt(*u as u64),
        Value::U32(u) => MyValue::UInt(*u as u64),
        Value::U64(u) => MyValue::UInt(*u),
        Value::U128(u) => MyValue::Bytes(u.to_string().into_bytes()),
        Value::F32(f) => MyValue::Float(*f),
        Value::F64(f) => MyValue::Double(*f),
        Value::Char(c) => MyValue::Bytes(c.to_string().into_bytes()),
        Value::Str(s) => MyValue::Bytes(s.as_bytes().to_vec()),
        Value::Bytes(b) => MyValue::Bytes(b.clone()),
        Value::Date(d) => to_mysql_date_value(*d, NaiveTime::default()),
        Value::Time(t) => MyValue::Time(
            false,
            0,
            t.hour() as u8,
            t.minute() as u8,
            t.second() as u8,
            t.nanosecond() / 1000,
        ),
        Value::DateTime(dt) => to_mysql_date_value(dt.date(), dt.time()),
        Value::DateTimeUtc(dt) => {
            let ndt = dt.naive_utc();
            to_mysql_date_value(ndt.date(), ndt.time())
        }
        Value::Decimal(d) => MyValue::Bytes(d.to_string().into_bytes()),
        Value::List(_) | Value::Map(_) => MyValue::Bytes(Vec::new()),
    }
}

fn to_mysql_date_value(d: NaiveDate, t: NaiveTime) -> MyValue {
    MyValue::Date(
        d.year() as u16,
        d.month() as u8,
        d.day() as u8,
        t.hour() as u8,
        t.minute() as u8,
        t.second() as u8,
        t.nanosecond() / 1000,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{NaiveDate, NaiveTime};

    #[test]
    fn test_date_conversion() {
        let date = NaiveDate::from_ymd_opt(2023, 10, 27).unwrap();
        let val = Value::Date(date);
        let my_val = to_mysql_value(&val);

        if let MyValue::Date(y, m, d, h, min, s, micro) = my_val {
            assert_eq!(y, 2023);
            assert_eq!(m, 10);
            assert_eq!(d, 27);
            assert_eq!(h, 0);
            assert_eq!(min, 0);
            assert_eq!(s, 0);
            assert_eq!(micro, 0);
        } else {
            panic!("Expected MyValue::Date");
        }

        let back = from_mysql_value(my_val);
        assert_eq!(back, val);
    }

    #[test]
    fn test_datetime_conversion() {
        let date = NaiveDate::from_ymd_opt(2023, 10, 27).unwrap();
        let time = NaiveTime::from_hms_micro_opt(12, 34, 56, 123456).unwrap();
        let dt = date.and_time(time);
        let val = Value::DateTime(dt);
        let my_val = to_mysql_value(&val);

        if let MyValue::Date(y, m, d, h, min, s, micro) = my_val {
            assert_eq!(y, 2023);
            assert_eq!(m, 10);
            assert_eq!(d, 27);
            assert_eq!(h, 12);
            assert_eq!(min, 34);
            assert_eq!(s, 56);
            assert_eq!(micro, 123456);
        } else {
            panic!("Expected MyValue::Date");
        }

        let back = from_mysql_value(my_val);
        assert_eq!(back, val);
    }

    #[test]
    fn test_time_conversion() {
        let time = NaiveTime::from_hms_micro_opt(12, 34, 56, 123456).unwrap();
        let val = Value::Time(time);
        let my_val = to_mysql_value(&val);

        if let MyValue::Time(neg, d, h, min, s, micro) = my_val {
            assert!(!neg);
            assert_eq!(d, 0);
            assert_eq!(h, 12);
            assert_eq!(min, 34);
            assert_eq!(s, 56);
            assert_eq!(micro, 123456);
        } else {
            panic!("Expected MyValue::Time");
        }

        let back = from_mysql_value(my_val);
        assert_eq!(back, val);
    }
}
