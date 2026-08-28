use std::io;

use serde::{de::DeserializeOwned, Serialize};

const FLOAT_BITS_KEY: &str = "__marklab_f64_bits";

pub(crate) fn encode<T: Serialize>(value: &T) -> io::Result<Box<[u8]>> {
    let mut value = serde_json::to_value(value).map_err(invalid_owned)?;
    encode_float_bits(&mut value);
    serde_json::to_vec_pretty(&value)
        .map(Vec::into_boxed_slice)
        .map_err(invalid_owned)
}

pub(crate) fn decode<T: DeserializeOwned>(bytes: &[u8]) -> io::Result<T> {
    let mut value: serde_json::Value = serde_json::from_slice(bytes).map_err(invalid_owned)?;
    decode_float_bits(&mut value)?;
    serde_json::from_value(value).map_err(invalid_owned)
}

fn encode_float_bits(value: &mut serde_json::Value) {
    match value {
        serde_json::Value::Number(number) if number.is_f64() => {
            let bits = number.as_f64().expect("f64 JSON number").to_bits();
            *value = serde_json::json!({ (FLOAT_BITS_KEY): bits });
        }
        serde_json::Value::Array(values) => {
            for value in values {
                encode_float_bits(value);
            }
        }
        serde_json::Value::Object(fields) => {
            for value in fields.values_mut() {
                encode_float_bits(value);
            }
        }
        _ => {}
    }
}

fn decode_float_bits(value: &mut serde_json::Value) -> io::Result<()> {
    match value {
        serde_json::Value::Array(values) => {
            for value in values {
                decode_float_bits(value)?;
            }
        }
        serde_json::Value::Object(fields)
            if fields.len() == 1 && fields.contains_key(FLOAT_BITS_KEY) =>
        {
            let bits = fields[FLOAT_BITS_KEY]
                .as_u64()
                .ok_or_else(|| invalid("exact f64 bit tag is invalid"))?;
            let number = serde_json::Number::from_f64(f64::from_bits(bits))
                .ok_or_else(|| invalid("exact f64 bit tag is non-finite"))?;
            *value = serde_json::Value::Number(number);
        }
        serde_json::Value::Object(fields) => {
            for value in fields.values_mut() {
                decode_float_bits(value)?;
            }
        }
        _ => {}
    }
    Ok(())
}

fn invalid(message: &'static str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message)
}

fn invalid_owned(error: impl std::fmt::Display) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, error.to_string())
}
