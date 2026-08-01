use std::fmt;

use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use thiserror::Error;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum CanonicalError {
    #[error("failed to serialize canonical value: {0}")]
    Serialization(String),
    #[error("floating-point numbers are not permitted in canonical values")]
    FloatingPoint,
}

macro_rules! digest_type {
    ($name:ident) => {
        #[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
        #[serde(transparent)]
        pub struct $name(String);

        impl $name {
            pub(crate) fn from_raw(value: String) -> Self {
                Self(value)
            }

            pub fn as_str(&self) -> &str {
                &self.0
            }

            pub fn is_valid(&self) -> bool {
                is_sha256(&self.0)
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str(&self.0)
            }
        }
    };
}

digest_type!(ActionDigest);
digest_type!(PolicyDigest);
digest_type!(ApprovalBindingDigest);

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(transparent)]
pub struct StateDigest(String);

impl StateDigest {
    pub fn parse(value: impl Into<String>) -> Option<Self> {
        let value = value.into();
        is_sha256(&value).then_some(Self(value))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub fn is_valid(&self) -> bool {
        is_sha256(&self.0)
    }
}

impl fmt::Display for StateDigest {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

pub(crate) fn canonical_json<T: Serialize>(value: &T) -> Result<Vec<u8>, CanonicalError> {
    let value = serde_json::to_value(value)
        .map_err(|error| CanonicalError::Serialization(error.to_string()))?;
    let mut output = String::new();
    write_value(&value, &mut output)?;
    Ok(output.into_bytes())
}

pub(crate) fn domain_digest(domain: &[u8], canonical: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(domain);
    hasher.update(canonical);
    format!("sha256:{}", hex::encode(hasher.finalize()))
}

pub(crate) fn is_sha256(value: &str) -> bool {
    value.len() == 71
        && value.starts_with("sha256:")
        && value[7..]
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn write_value(value: &Value, output: &mut String) -> Result<(), CanonicalError> {
    match value {
        Value::Null => output.push_str("null"),
        Value::Bool(value) => output.push_str(if *value { "true" } else { "false" }),
        Value::Number(value) => {
            if value.as_i64().is_none() && value.as_u64().is_none() {
                return Err(CanonicalError::FloatingPoint);
            }
            output.push_str(&value.to_string());
        }
        Value::String(value) => output.push_str(
            &serde_json::to_string(value)
                .map_err(|error| CanonicalError::Serialization(error.to_string()))?,
        ),
        Value::Array(values) => {
            output.push('[');
            for (index, value) in values.iter().enumerate() {
                if index > 0 {
                    output.push(',');
                }
                write_value(value, output)?;
            }
            output.push(']');
        }
        Value::Object(values) => {
            output.push('{');
            let mut keys: Vec<_> = values.keys().collect();
            keys.sort_unstable();
            for (index, key) in keys.into_iter().enumerate() {
                if index > 0 {
                    output.push(',');
                }
                output.push_str(
                    &serde_json::to_string(key)
                        .map_err(|error| CanonicalError::Serialization(error.to_string()))?,
                );
                output.push(':');
                write_value(&values[key], output)?;
            }
            output.push('}');
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use serde::Serialize;

    use super::*;

    #[derive(Serialize)]
    struct Fixture {
        z: u8,
        a: BTreeMap<String, String>,
    }

    #[test]
    fn canonical_writer_has_a_stable_domain_separated_vector() {
        let fixture = Fixture {
            z: 7,
            a: BTreeMap::from([("z".into(), "last".into()), ("a".into(), "first".into())]),
        };
        let bytes = canonical_json(&fixture).unwrap();
        assert_eq!(bytes, br#"{"a":{"a":"first","z":"last"},"z":7}"#);
        assert_eq!(
            domain_digest(b"vea\0test\0v1\0", &bytes),
            "sha256:4c4579e398f7e74b22feb47db9bd0f123cb3f3c399eda3c16ad67339dd0712c8"
        );
    }
}
