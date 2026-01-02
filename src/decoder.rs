//! WeakAura decoding pipeline
//!
//! !WA:2! string -> DecodeForPrint -> DEFLATE decompress -> LibSerialize -> LuaValue -> JSON

use crate::encodeforprint::{decode_for_print, EncodeForPrintError};
use crate::libserialize::{deserialize, SerializeError};
use crate::types::LuaValue;
use flate2::read::{DeflateDecoder, ZlibDecoder};
use std::io::Read;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum DecodeError {
    #[error("Invalid WA string format: missing !WA:2! prefix")]
    InvalidPrefix,
    #[error("Unsupported WA version: {0}")]
    UnsupportedVersion(u8),
    #[error("Decode error: {0}")]
    DecodeForPrint(#[from] EncodeForPrintError),
    #[error("Decompression error: {0}")]
    Decompression(#[from] std::io::Error),
    #[error("Deserialization error: {0}")]
    Deserialize(#[from] SerializeError),
    #[error("JSON serialization error: {0}")]
    JsonSerialize(#[from] serde_json::Error),
}

/// Decode a WeakAura import string to JSON
pub fn decode(import_string: &str) -> Result<String, DecodeError> {
    let value = decode_to_value(import_string)?;
    let json = value.to_json();
    Ok(serde_json::to_string_pretty(&json)?)
}

/// Decode a WeakAura import string to LuaValue
pub fn decode_to_value(import_string: &str) -> Result<LuaValue, DecodeError> {
    let trimmed = import_string.trim();

    // Check for !WA:X! prefix
    if !trimmed.starts_with("!WA:") {
        return Err(DecodeError::InvalidPrefix);
    }

    // Find version and encoded data
    let after_prefix = &trimmed[4..];
    let exclaim_pos = after_prefix
        .find('!')
        .ok_or(DecodeError::InvalidPrefix)?;

    let version_str = &after_prefix[..exclaim_pos];
    let version: u8 = version_str
        .parse()
        .map_err(|_| DecodeError::InvalidPrefix)?;

    if version < 2 {
        return Err(DecodeError::UnsupportedVersion(version));
    }

    let encoded = &after_prefix[exclaim_pos + 1..];

    // Decode from custom base64
    let compressed = decode_for_print(encoded)?;

    // Try DEFLATE first, then zlib if that fails
    let decompressed = {
        let mut decoder = DeflateDecoder::new(&compressed[..]);
        let mut buf = Vec::new();
        match decoder.read_to_end(&mut buf) {
            Ok(_) => buf,
            Err(_) => {
                let mut decoder = ZlibDecoder::new(&compressed[..]);
                let mut buf = Vec::new();
                decoder.read_to_end(&mut buf)?;
                buf
            }
        }
    };

    // Deserialize
    let value = deserialize(&decompressed)?;

    Ok(value)
}

/// Validate a WeakAura import string
pub fn validate(import_string: &str) -> ValidationResult {
    match decode_to_value(import_string) {
        Ok(value) => {
            let mut result = ValidationResult {
                valid: true,
                errors: Vec::new(),
                warnings: Vec::new(),
            };

            // Check for required fields
            if let LuaValue::Table(pairs) = &value {
                let has_d = pairs.iter().any(|(k, _)| {
                    matches!(k, LuaValue::String(s) if s == "d")
                });
                if !has_d {
                    result.warnings.push("Missing 'd' field (aura definition)".to_string());
                }
            } else {
                result.warnings.push("Root value is not a table".to_string());
            }

            result
        }
        Err(e) => ValidationResult {
            valid: false,
            errors: vec![e.to_string()],
            warnings: Vec::new(),
        },
    }
}

#[derive(Debug)]
pub struct ValidationResult {
    pub valid: bool,
    pub errors: Vec<String>,
    pub warnings: Vec<String>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::encoder::encode;

    #[test]
    fn test_roundtrip() {
        let json = r#"{"d":{"id":"Test","regionType":"icon","width":64,"height":64}}"#;
        let encoded = encode(json).unwrap();
        println!("Encoded: {}", encoded);

        let decoded = decode(&encoded).unwrap();
        println!("Decoded: {}", decoded);

        // Parse both as JSON and compare structure
        let original: serde_json::Value = serde_json::from_str(json).unwrap();
        let decoded_val: serde_json::Value = serde_json::from_str(&decoded).unwrap();

        // Check key fields match
        assert_eq!(
            original["d"]["id"],
            decoded_val["d"]["id"]
        );
        assert_eq!(
            original["d"]["regionType"],
            decoded_val["d"]["regionType"]
        );
    }

    #[test]
    fn test_invalid_prefix() {
        let result = decode("invalid string");
        assert!(matches!(result, Err(DecodeError::InvalidPrefix)));
    }
}
