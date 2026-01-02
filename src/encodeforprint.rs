//! Custom base64 encoding for WeakAuras (EncodeForPrint/DecodeForPrint)
//!
//! Uses a custom 64-character alphabet:
//! - 0-25: a-z (lowercase)
//! - 26-51: A-Z (uppercase)
//! - 52-61: 0-9
//! - 62: (
//! - 63: )

use thiserror::Error;

/// The 64-character alphabet used by LibDeflate's EncodeForPrint
const ENCODE_ALPHABET: &[u8; 64] = b"abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789()";

/// Reverse lookup table: ASCII char -> 6-bit value (255 = invalid)
const DECODE_TABLE: [u8; 128] = {
    let mut table = [255u8; 128];

    // a-z = 0-25
    let mut i = 0;
    while i < 26 {
        table[(b'a' + i) as usize] = i;
        i += 1;
    }

    // A-Z = 26-51
    i = 0;
    while i < 26 {
        table[(b'A' + i) as usize] = 26 + i;
        i += 1;
    }

    // 0-9 = 52-61
    i = 0;
    while i < 10 {
        table[(b'0' + i) as usize] = 52 + i;
        i += 1;
    }

    // ( = 62, ) = 63
    table[b'(' as usize] = 62;
    table[b')' as usize] = 63;

    table
};

#[derive(Error, Debug)]
pub enum EncodeForPrintError {
    #[error("Invalid character in input: {0}")]
    InvalidChar(char),
    #[error("Input too short")]
    InputTooShort,
}

/// Encode bytes to EncodeForPrint format (custom base64)
///
/// Uses LITTLE-ENDIAN byte order (unlike standard base64)
/// Takes 3 bytes at a time, converts to 4 characters.
pub fn encode_for_print(data: &[u8]) -> String {
    let mut result = String::with_capacity((data.len() * 4 + 2) / 3);

    let mut i = 0;
    while i + 2 < data.len() {
        // Take 3 bytes = 24 bits (LITTLE ENDIAN: x1 + x2*256 + x3*65536)
        let x1 = data[i] as u32;
        let x2 = data[i + 1] as u32;
        let x3 = data[i + 2] as u32;

        let cache = x1 + x2 * 256 + x3 * 65536;

        // Extract 4 groups of 6 bits each (from low to high)
        let b1 = (cache % 64) as usize;
        let b2 = ((cache / 64) % 64) as usize;
        let b3 = ((cache / 4096) % 64) as usize;
        let b4 = ((cache / 262144) % 64) as usize;

        result.push(ENCODE_ALPHABET[b1] as char);
        result.push(ENCODE_ALPHABET[b2] as char);
        result.push(ENCODE_ALPHABET[b3] as char);
        result.push(ENCODE_ALPHABET[b4] as char);

        i += 3;
    }

    // Handle remaining 1 or 2 bytes
    let remaining = data.len() - i;
    if remaining > 0 {
        let mut cache: u32 = 0;
        let mut cache_bitlen = 0;

        for j in 0..remaining {
            cache += (data[i + j] as u32) << cache_bitlen;
            cache_bitlen += 8;
        }

        // Output 6-bit chunks until we've covered all bits
        while cache_bitlen > 0 {
            let bit6 = (cache % 64) as usize;
            result.push(ENCODE_ALPHABET[bit6] as char);
            cache /= 64;
            cache_bitlen -= 6;
        }
    }

    result
}

/// Decode EncodeForPrint format back to bytes
///
/// Uses LITTLE-ENDIAN byte order (unlike standard base64)
pub fn decode_for_print(data: &str) -> Result<Vec<u8>, EncodeForPrintError> {
    // Filter out whitespace and control characters
    let clean: Vec<u8> = data
        .bytes()
        .filter(|&b| b > 32 && b < 127)
        .collect();

    if clean.is_empty() {
        return Ok(Vec::new());
    }

    let mut result = Vec::with_capacity((clean.len() * 3) / 4);
    let strlen = clean.len();
    let strlen_minus_3 = if strlen >= 4 { strlen - 3 } else { 0 };

    let mut i = 0;

    // Process 4 characters at a time -> 3 bytes
    while i < strlen_minus_3 {
        let x1 = decode_char(clean[i])? as u32;
        let x2 = decode_char(clean[i + 1])? as u32;
        let x3 = decode_char(clean[i + 2])? as u32;
        let x4 = decode_char(clean[i + 3])? as u32;

        // LITTLE ENDIAN: cache = x1 + x2*64 + x3*4096 + x4*262144
        let cache = x1 + x2 * 64 + x3 * 4096 + x4 * 262144;

        // Extract 3 bytes (little endian)
        let b1 = (cache % 256) as u8;
        let b2 = ((cache / 256) % 256) as u8;
        let b3 = ((cache / 65536) % 256) as u8;

        result.push(b1);
        result.push(b2);
        result.push(b3);

        i += 4;
    }

    // Handle remaining characters
    if i < strlen {
        let mut cache: u32 = 0;
        let mut cache_bitlen = 0;

        while i < strlen {
            let x = decode_char(clean[i])? as u32;
            cache += x << cache_bitlen;
            cache_bitlen += 6;
            i += 1;
        }

        // Extract bytes while we have at least 8 bits
        while cache_bitlen >= 8 {
            let byte = (cache % 256) as u8;
            result.push(byte);
            cache /= 256;
            cache_bitlen -= 8;
        }
    }

    Ok(result)
}

#[inline]
fn decode_char(c: u8) -> Result<u8, EncodeForPrintError> {
    if c >= 128 {
        return Err(EncodeForPrintError::InvalidChar(c as char));
    }
    let val = DECODE_TABLE[c as usize];
    if val == 255 {
        return Err(EncodeForPrintError::InvalidChar(c as char));
    }
    Ok(val)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_roundtrip() {
        let original = b"Hello, World! This is a test of the WeakAura encoding.";
        let encoded = encode_for_print(original);
        let decoded = decode_for_print(&encoded).unwrap();
        assert_eq!(original.to_vec(), decoded);
    }

    #[test]
    fn test_empty() {
        let encoded = encode_for_print(b"");
        assert_eq!(encoded, "");
        let decoded = decode_for_print("").unwrap();
        assert!(decoded.is_empty());
    }

    #[test]
    fn test_single_byte() {
        let original = b"A";
        let encoded = encode_for_print(original);
        let decoded = decode_for_print(&encoded).unwrap();
        assert_eq!(original.to_vec(), decoded);
    }

    #[test]
    fn test_two_bytes() {
        let original = b"AB";
        let encoded = encode_for_print(original);
        let decoded = decode_for_print(&encoded).unwrap();
        assert_eq!(original.to_vec(), decoded);
    }

    #[test]
    fn test_alphabet_chars() {
        // Verify all alphabet chars are valid
        for &c in ENCODE_ALPHABET.iter() {
            let val = decode_char(c).unwrap();
            assert!(val < 64);
        }
    }
}
