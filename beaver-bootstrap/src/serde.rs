use serde::de::Error;
use serde::{Deserialize, Deserializer};

pub fn non_empty<'de, D>(des: D) -> Result<String, D::Error>
where
    D: Deserializer<'de>,
{
    let s = String::deserialize(des)?;
    if s.trim().is_empty() {
        return Err(D::Error::custom("value must not be empty"));
    }
    Ok(s)
}
