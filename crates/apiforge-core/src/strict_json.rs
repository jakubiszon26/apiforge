//! serde_json::Value normally overwrites duplicate keys. An importer must reject
//! ambiguity before converting to a DTO, including inside user-supplied literals.
use serde::{
    Deserialize, Deserializer,
    de::{self, MapAccess, SeqAccess, Visitor},
};
use serde_json::{Map, Number, Value};
use std::fmt;

/// Parse a typed JSON payload without silently overwriting duplicate keys,
/// including keys nested in user-provided maps and literal values. Callers must
/// bound input bytes and sanitise parse errors before showing diagnostics.
pub fn from_slice<T: serde::de::DeserializeOwned>(bytes: &[u8]) -> Result<T, serde_json::Error> {
    let value = serde_json::from_slice::<StrictValue>(bytes)?.0;
    serde_json::from_value(value)
}

pub(crate) struct StrictValue(pub Value);
impl<'de> Deserialize<'de> for StrictValue {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        deserializer.deserialize_any(StrictVisitor).map(Self)
    }
}
struct StrictVisitor;
impl<'de> Visitor<'de> for StrictVisitor {
    type Value = Value;
    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_str("unambiguous JSON")
    }
    fn visit_bool<E: de::Error>(self, value: bool) -> Result<Value, E> {
        Ok(Value::Bool(value))
    }
    fn visit_i64<E: de::Error>(self, value: i64) -> Result<Value, E> {
        Ok(Value::Number(value.into()))
    }
    fn visit_u64<E: de::Error>(self, value: u64) -> Result<Value, E> {
        Ok(Value::Number(value.into()))
    }
    fn visit_f64<E: de::Error>(self, value: f64) -> Result<Value, E> {
        Number::from_f64(value)
            .map(Value::Number)
            .ok_or_else(|| E::custom("non-finite number"))
    }
    fn visit_str<E: de::Error>(self, value: &str) -> Result<Value, E> {
        Ok(Value::String(value.to_owned()))
    }
    fn visit_string<E: de::Error>(self, value: String) -> Result<Value, E> {
        Ok(Value::String(value))
    }
    fn visit_unit<E: de::Error>(self) -> Result<Value, E> {
        Ok(Value::Null)
    }
    fn visit_seq<A: SeqAccess<'de>>(self, mut array: A) -> Result<Value, A::Error> {
        let mut result = Vec::new();
        while let Some(StrictValue(value)) = array.next_element()? {
            result.push(value);
        }
        Ok(Value::Array(result))
    }
    fn visit_map<A: MapAccess<'de>>(self, mut object: A) -> Result<Value, A::Error> {
        let mut result = Map::new();
        while let Some(key) = object.next_key::<String>()? {
            if result.contains_key(&key) {
                return Err(de::Error::custom("duplicate JSON key"));
            }
            result.insert(key, object.next_value::<StrictValue>()?.0);
        }
        Ok(Value::Object(result))
    }
}
