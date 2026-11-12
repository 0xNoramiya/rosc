//! Token amounts cross JSON as decimal strings, as NEP-141 does.
use serde::{Deserialize, Deserializer, Serializer};

pub fn serialize<S: Serializer>(v: &u128, s: S) -> Result<S::Ok, S::Error> {
    s.collect_str(v)
}

pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<u128, D::Error> {
    let s = String::deserialize(d)?;
    s.parse().map_err(serde::de::Error::custom)
}
