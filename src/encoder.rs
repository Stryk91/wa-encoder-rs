//! WeakAura encoding pipeline
//!
//! JSON -> LuaValue -> LibSerialize -> DEFLATE -> EncodeForPrint -> !WA:2!

use crate::encodeforprint::encode_for_print;
use crate::libserialize::serialize;
use crate::types::LuaValue;
use flate2::write::DeflateEncoder;
use flate2::Compression;
use serde_json::Value;
use std::io::Write;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum EncodeError {
    #[error("JSON parse error: {0}")]
    JsonParse(#[from] serde_json::Error),
    #[error("Compression error: {0}")]
    Compression(#[from] std::io::Error),
}

/// Encode a JSON string to a WeakAura import string
pub fn encode(json: &str) -> Result<String, EncodeError> {
    // Parse JSON
    let mut value: Value = serde_json::from_str(json)?;

    // Auto-add required WeakAura metadata fields if missing
    if let Value::Object(ref mut obj) = value {
        if !obj.contains_key("m") {
            obj.insert("m".to_string(), Value::String("d".to_string()));
        }
        if !obj.contains_key("s") {
            obj.insert("s".to_string(), Value::String("5.20.7".to_string()));
        }
        if !obj.contains_key("v") {
            obj.insert("v".to_string(), Value::Number(2000.into()));
        }
    }

    // Convert to LuaValue
    let lua_value = LuaValue::from_json(&value);

    // Serialize to bytes
    let serialized = serialize(&lua_value);

    // Compress with DEFLATE
    let mut encoder = DeflateEncoder::new(Vec::new(), Compression::best());
    encoder.write_all(&serialized)?;
    let compressed = encoder.finish()?;

    // Encode for print
    let encoded = encode_for_print(&compressed);

    // Add prefix
    Ok(format!("!WA:2!{}", encoded))
}

/// Encode a LuaValue directly to a WeakAura import string
pub fn encode_value(value: &LuaValue) -> Result<String, EncodeError> {
    // Serialize to bytes
    let serialized = serialize(value);

    // Compress with DEFLATE
    let mut encoder = DeflateEncoder::new(Vec::new(), Compression::best());
    encoder.write_all(&serialized)?;
    let compressed = encoder.finish()?;

    // Encode for print
    let encoded = encode_for_print(&compressed);

    // Add prefix
    Ok(format!("!WA:2!{}", encoded))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_encode_simple() {
        let json = r#"{"d":{"id":"Test","regionType":"icon"}}"#;
        let result = encode(json).unwrap();
        assert!(result.starts_with("!WA:2!"));
        assert!(result.len() > 10);
    }
}
