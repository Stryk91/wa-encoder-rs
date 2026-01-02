//! LibSerialize format implementation
//!
//! This implements the binary serialization format used by LibSerialize.
//! Based on https://github.com/rossnichols/LibSerialize

use crate::types::LuaValue;
use std::collections::HashMap;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum SerializeError {
    #[error("Invalid type byte: {0:#04x}")]
    InvalidTypeByte(u8),
    #[error("Unexpected end of input")]
    UnexpectedEnd,
    #[error("Invalid string encoding")]
    InvalidString,
    #[error("Invalid table reference: {0}")]
    InvalidTableRef(usize),
    #[error("Invalid string reference: {0}")]
    InvalidStringRef(usize),
    #[error("Unsupported float value")]
    UnsupportedFloat,
    #[error("Invalid serialization version: {0}")]
    InvalidVersion(u8),
}

// LibSerialize constants
const SERIALIZATION_VERSION: u8 = 1;

// Shifts for encoding
const EMBEDDED_INDEX_SHIFT: u8 = 4;
const EMBEDDED_COUNT_SHIFT: u8 = 16;
const READER_INDEX_SHIFT: u8 = 8;

// Embedded type indices
const EMBEDDED_STRING: u8 = 0;
const EMBEDDED_TABLE: u8 = 1;
const EMBEDDED_ARRAY: u8 = 2;
const EMBEDDED_MIXED: u8 = 3;

// Reader indices (multiplied by 8 to get actual type byte)
const READER_NIL: u8 = 0;
const READER_NUM_16_POS: u8 = 1;
const READER_NUM_16_NEG: u8 = 2;
const READER_NUM_24_POS: u8 = 3;
const READER_NUM_24_NEG: u8 = 4;
const READER_NUM_32_POS: u8 = 5;
const READER_NUM_32_NEG: u8 = 6;
const READER_NUM_64_POS: u8 = 7;
const READER_NUM_64_NEG: u8 = 8;
const READER_NUM_FLOAT: u8 = 9;
const READER_NUM_FLOATSTR_POS: u8 = 10;
const READER_NUM_FLOATSTR_NEG: u8 = 11;
const READER_BOOL_T: u8 = 12;
const READER_BOOL_F: u8 = 13;
const READER_STR_8: u8 = 14;
const READER_STR_16: u8 = 15;
const READER_STR_24: u8 = 16;
const READER_TABLE_8: u8 = 17;
const READER_TABLE_16: u8 = 18;
const READER_TABLE_24: u8 = 19;
const READER_ARRAY_8: u8 = 20;
const READER_ARRAY_16: u8 = 21;
const READER_ARRAY_24: u8 = 22;
const READER_MIXED_8: u8 = 23;
const READER_MIXED_16: u8 = 24;
const READER_MIXED_24: u8 = 25;
const READER_STRINGREF_8: u8 = 26;
const READER_STRINGREF_16: u8 = 27;
const READER_STRINGREF_24: u8 = 28;
const READER_TABLEREF_8: u8 = 29;
const READER_TABLEREF_16: u8 = 30;
const READER_TABLEREF_24: u8 = 31;

/// Serializer state
pub struct Serializer {
    output: Vec<u8>,
    string_refs: HashMap<String, usize>,
    table_count: usize,
}

impl Serializer {
    pub fn new() -> Self {
        Self {
            output: Vec::new(),
            string_refs: HashMap::new(),
            table_count: 0,
        }
    }

    pub fn serialize(mut self, value: &LuaValue) -> Vec<u8> {
        // Write version byte first
        self.output.push(SERIALIZATION_VERSION);
        self.write_value(value);
        self.output
    }

    fn write_byte(&mut self, b: u8) {
        self.output.push(b);
    }

    fn write_int_bytes(&mut self, val: u64, count: usize) {
        // LibSerialize uses BIG-ENDIAN: high byte first
        for i in (0..count).rev() {
            self.output.push(((val >> (i * 8)) & 0xFF) as u8);
        }
    }

    fn write_value(&mut self, value: &LuaValue) {
        match value {
            LuaValue::Nil => {
                self.write_byte(READER_INDEX_SHIFT * READER_NIL);
            }
            LuaValue::Bool(true) => {
                self.write_byte(READER_INDEX_SHIFT * READER_BOOL_T);
            }
            LuaValue::Bool(false) => {
                self.write_byte(READER_INDEX_SHIFT * READER_BOOL_F);
            }
            LuaValue::Int(i) => self.write_number(*i as f64),
            LuaValue::Float(f) => self.write_number(*f),
            LuaValue::String(s) => self.write_string(s),
            LuaValue::Array(arr) => self.write_array(arr),
            LuaValue::Table(pairs) => self.write_table(pairs),
        }
    }

    fn write_number(&mut self, num: f64) {
        // Check if it's a small integer (0-127)
        if num.fract() == 0.0 && num >= 0.0 && num <= 127.0 {
            // Small positive int: encoded as (num * 2 + 1)
            self.write_byte((num as u8) * 2 + 1);
            return;
        }

        // Check for negative small int or need larger encoding
        if num.fract() == 0.0 {
            let i = num as i64;
            let abs = i.unsigned_abs();
            let sign = if i < 0 { 1u8 } else { 0u8 };

            // 12-bit check: -4095 to 4095 (excluding small ints already handled)
            if abs <= 4095 {
                // Two-byte encoding
                let lower = ((abs as u8) << 4) | (sign << 3) | 0x04;
                let upper = (abs >> 4) as u8;
                self.write_byte(lower);
                self.write_byte(upper);
                return;
            }

            // Larger int encoding
            let required = if abs <= 0xFFFF { 2 }
            else if abs <= 0xFFFFFF { 3 }
            else if abs <= 0xFFFFFFFF { 4 }
            else { 7 };

            let reader_idx = if i >= 0 {
                match required {
                    2 => READER_NUM_16_POS,
                    3 => READER_NUM_24_POS,
                    4 => READER_NUM_32_POS,
                    _ => READER_NUM_64_POS,
                }
            } else {
                match required {
                    2 => READER_NUM_16_NEG,
                    3 => READER_NUM_24_NEG,
                    4 => READER_NUM_32_NEG,
                    _ => READER_NUM_64_NEG,
                }
            };

            self.write_byte(READER_INDEX_SHIFT * reader_idx);
            self.write_int_bytes(abs, required);
        } else {
            // Float encoding
            self.write_byte(READER_INDEX_SHIFT * READER_NUM_FLOAT);
            self.output.extend_from_slice(&num.to_be_bytes());
        }
    }

    fn write_string(&mut self, s: &str) {
        let len = s.len();

        // Check for string reference
        if len > 2 {
            if let Some(&ref_idx) = self.string_refs.get(s) {
                // LibSerialize uses 1-based indices
                let ref_1based = ref_idx + 1;
                let required = if ref_1based <= 0xFF { 1 }
                else if ref_1based <= 0xFFFF { 2 }
                else { 3 };

                let reader_idx = match required {
                    1 => READER_STRINGREF_8,
                    2 => READER_STRINGREF_16,
                    _ => READER_STRINGREF_24,
                };
                self.write_byte(READER_INDEX_SHIFT * reader_idx);
                self.write_int_bytes(ref_1based as u64, required);
                return;
            }
            // Add to refs (store 0-based internally)
            let ref_idx = self.string_refs.len();
            self.string_refs.insert(s.to_string(), ref_idx);
        }

        // Check if we can use embedded format (len <= 15)
        if len <= 15 {
            // Embedded string: count * 16 + type * 4 + 2
            self.write_byte((len as u8) * 16 + EMBEDDED_STRING * 4 + 2);
            self.output.extend_from_slice(s.as_bytes());
        } else {
            let required = if len <= 0xFF { 1 }
            else if len <= 0xFFFF { 2 }
            else { 3 };

            let reader_idx = match required {
                1 => READER_STR_8,
                2 => READER_STR_16,
                _ => READER_STR_24,
            };
            self.write_byte(READER_INDEX_SHIFT * reader_idx);
            self.write_int_bytes(len as u64, required);
            self.output.extend_from_slice(s.as_bytes());
        }
    }

    fn write_array(&mut self, arr: &[LuaValue]) {
        let len = arr.len();
        self.table_count += 1;

        if len <= 15 {
            // Embedded array
            self.write_byte((len as u8) * 16 + EMBEDDED_ARRAY * 4 + 2);
        } else {
            let required = if len <= 0xFF { 1 }
            else if len <= 0xFFFF { 2 }
            else { 3 };

            let reader_idx = match required {
                1 => READER_ARRAY_8,
                2 => READER_ARRAY_16,
                _ => READER_ARRAY_24,
            };
            self.write_byte(READER_INDEX_SHIFT * reader_idx);
            self.write_int_bytes(len as u64, required);
        }

        for value in arr {
            self.write_value(value);
        }
    }

    fn write_table(&mut self, pairs: &[(LuaValue, LuaValue)]) {
        let len = pairs.len();
        self.table_count += 1;

        if len <= 15 {
            // Embedded table
            self.write_byte((len as u8) * 16 + EMBEDDED_TABLE * 4 + 2);
        } else {
            let required = if len <= 0xFF { 1 }
            else if len <= 0xFFFF { 2 }
            else { 3 };

            let reader_idx = match required {
                1 => READER_TABLE_8,
                2 => READER_TABLE_16,
                _ => READER_TABLE_24,
            };
            self.write_byte(READER_INDEX_SHIFT * reader_idx);
            self.write_int_bytes(len as u64, required);
        }

        for (key, value) in pairs {
            self.write_value(key);
            self.write_value(value);
        }
    }
}

/// Deserializer state
pub struct Deserializer<'a> {
    input: &'a [u8],
    pos: usize,
    string_refs: Vec<String>,
    table_refs: Vec<LuaValue>,
}

impl<'a> Deserializer<'a> {
    pub fn new(input: &'a [u8]) -> Self {
        Self {
            input,
            pos: 0,
            string_refs: Vec::new(),
            table_refs: Vec::new(),
        }
    }

    pub fn deserialize(mut self) -> Result<LuaValue, SerializeError> {
        // Read and verify version
        let version = self.read_u8()?;
        if version > 2 {
            return Err(SerializeError::InvalidVersion(version));
        }
        self.read_value()
    }

    fn read_u8(&mut self) -> Result<u8, SerializeError> {
        if self.pos >= self.input.len() {
            return Err(SerializeError::UnexpectedEnd);
        }
        let b = self.input[self.pos];
        self.pos += 1;
        Ok(b)
    }

    fn read_bytes(&mut self, n: usize) -> Result<&'a [u8], SerializeError> {
        if self.pos + n > self.input.len() {
            return Err(SerializeError::UnexpectedEnd);
        }
        let bytes = &self.input[self.pos..self.pos + n];
        self.pos += n;
        Ok(bytes)
    }

    fn read_int(&mut self, count: usize) -> Result<u64, SerializeError> {
        let bytes = self.read_bytes(count)?;
        // LibSerialize uses BIG-ENDIAN: first byte is high bits
        let mut val: u64 = 0;
        for &b in bytes.iter() {
            val = (val << 8) | (b as u64);
        }
        Ok(val)
    }

    fn read_value(&mut self) -> Result<LuaValue, SerializeError> {
        let byte = self.read_u8()?;

        // Check for small integer (LSB = 1)
        if byte & 1 == 1 {
            let val = (byte - 1) / 2;
            return Ok(LuaValue::Int(val as i64));
        }

        // Check for embedded type (bits 0-1 = 10, i.e., & 3 == 2)
        if byte & 3 == 2 {
            let count = (byte >> 4) as usize;
            let type_idx = (byte >> 2) & 3;

            return match type_idx {
                0 => {
                    // Embedded STRING
                    let bytes = self.read_bytes(count)?;
                    let s = String::from_utf8_lossy(bytes).to_string();
                    if count > 2 {
                        self.string_refs.push(s.clone());
                    }
                    Ok(LuaValue::String(s))
                }
                1 => {
                    // Embedded TABLE
                    self.read_table(count)
                }
                2 => {
                    // Embedded ARRAY
                    self.read_array(count)
                }
                3 => {
                    // Embedded MIXED
                    // count contains two 2-bit counts that are one less than true count
                    let arr_count = (count & 3) + 1;
                    let hash_count = (count >> 2) + 1;
                    self.read_mixed(arr_count, hash_count)
                }
                _ => unreachable!(),
            }
        }
        // Check for 12-bit integer (bits 0-2 = 100, i.e., & 7 == 4)
        else if byte & 7 == 4 {
            let sign = (byte >> 3) & 1;
            let lower = (byte >> 4) as u16;
            let upper = self.read_u8()? as u16;
            let val = lower | (upper << 4);
            if sign == 1 {
                Ok(LuaValue::Int(-(val as i64)))
            } else {
                Ok(LuaValue::Int(val as i64))
            }
        }
        // Reader type (bits 0-2 = 000, i.e., & 7 == 0)
        else if byte & 7 == 0 {
            let reader_idx = byte >> 3;
            self.read_by_reader_index(reader_idx)
        } else {
            Err(SerializeError::InvalidTypeByte(byte))
        }
    }

    fn read_by_reader_index(&mut self, idx: u8) -> Result<LuaValue, SerializeError> {
        match idx {
            0 => Ok(LuaValue::Nil),  // NIL
            1 => {
                // NUM_16_POS
                let val = self.read_int(2)?;
                Ok(LuaValue::Int(val as i64))
            }
            2 => {
                // NUM_16_NEG
                let val = self.read_int(2)?;
                Ok(LuaValue::Int(-(val as i64)))
            }
            3 => {
                // NUM_24_POS
                let val = self.read_int(3)?;
                Ok(LuaValue::Int(val as i64))
            }
            4 => {
                // NUM_24_NEG
                let val = self.read_int(3)?;
                Ok(LuaValue::Int(-(val as i64)))
            }
            5 => {
                // NUM_32_POS
                let val = self.read_int(4)?;
                Ok(LuaValue::Int(val as i64))
            }
            6 => {
                // NUM_32_NEG
                let val = self.read_int(4)?;
                Ok(LuaValue::Int(-(val as i64)))
            }
            7 => {
                // NUM_64_POS (actually 7 bytes)
                let val = self.read_int(7)?;
                Ok(LuaValue::Int(val as i64))
            }
            8 => {
                // NUM_64_NEG (actually 7 bytes)
                let val = self.read_int(7)?;
                Ok(LuaValue::Int(-(val as i64)))
            }
            9 => {
                // NUM_FLOAT (8 bytes big-endian)
                let bytes = self.read_bytes(8)?;
                let f = f64::from_be_bytes(bytes.try_into().unwrap());
                Ok(LuaValue::Float(f))
            }
            10 => {
                // NUM_FLOATSTR_POS
                let len = self.read_u8()? as usize;
                let bytes = self.read_bytes(len)?;
                let s = std::str::from_utf8(bytes).map_err(|_| SerializeError::InvalidString)?;
                let f: f64 = s.parse().map_err(|_| SerializeError::UnsupportedFloat)?;
                Ok(LuaValue::Float(f))
            }
            11 => {
                // NUM_FLOATSTR_NEG
                let len = self.read_u8()? as usize;
                let bytes = self.read_bytes(len)?;
                let s = std::str::from_utf8(bytes).map_err(|_| SerializeError::InvalidString)?;
                let f: f64 = s.parse().map_err(|_| SerializeError::UnsupportedFloat)?;
                Ok(LuaValue::Float(-f))
            }
            12 => Ok(LuaValue::Bool(true)),   // BOOL_T
            13 => Ok(LuaValue::Bool(false)),  // BOOL_F
            14 => {
                // STR_8
                let len = self.read_u8()? as usize;
                self.read_string(len)
            }
            15 => {
                // STR_16 - BIG-ENDIAN
                let len = self.read_int(2)? as usize;
                self.read_string(len)
            }
            16 => {
                // STR_24 - BIG-ENDIAN
                let len = self.read_int(3)? as usize;
                self.read_string(len)
            }
            17 => {
                // TABLE_8
                let len = self.read_u8()? as usize;
                self.read_table(len)
            }
            18 => {
                // TABLE_16
                let len = self.read_int(2)? as usize;
                self.read_table(len)
            }
            19 => {
                // TABLE_24
                let len = self.read_int(3)? as usize;
                self.read_table(len)
            }
            20 => {
                // ARRAY_8
                let len = self.read_u8()? as usize;
                self.read_array(len)
            }
            21 => {
                // ARRAY_16
                let len = self.read_int(2)? as usize;
                self.read_array(len)
            }
            22 => {
                // ARRAY_24
                let len = self.read_int(3)? as usize;
                self.read_array(len)
            }
            23 => {
                // MIXED_8
                let arr_len = self.read_u8()? as usize;
                let hash_len = self.read_u8()? as usize;
                self.read_mixed(arr_len, hash_len)
            }
            24 => {
                // MIXED_16
                let arr_len = self.read_int(2)? as usize;
                let hash_len = self.read_int(2)? as usize;
                self.read_mixed(arr_len, hash_len)
            }
            25 => {
                // MIXED_24
                let arr_len = self.read_int(3)? as usize;
                let hash_len = self.read_int(3)? as usize;
                self.read_mixed(arr_len, hash_len)
            }
            26 => {
                // STRINGREF_8 (1-based index in Lua)
                let idx = self.read_u8()? as usize;
                let idx0 = idx.saturating_sub(1);
                self.string_refs
                    .get(idx0)
                    .cloned()
                    .map(LuaValue::String)
                    .ok_or(SerializeError::InvalidStringRef(idx))
            }
            27 => {
                // STRINGREF_16 (1-based index in Lua)
                let idx = self.read_int(2)? as usize;
                let idx0 = idx.saturating_sub(1);
                self.string_refs
                    .get(idx0)
                    .cloned()
                    .map(LuaValue::String)
                    .ok_or(SerializeError::InvalidStringRef(idx))
            }
            28 => {
                // STRINGREF_24 (1-based index in Lua)
                let idx = self.read_int(3)? as usize;
                let idx0 = idx.saturating_sub(1);
                self.string_refs
                    .get(idx0)
                    .cloned()
                    .map(LuaValue::String)
                    .ok_or(SerializeError::InvalidStringRef(idx))
            }
            29 => {
                // TABLEREF_8 (1-based index in Lua)
                let idx = self.read_u8()? as usize;
                let idx0 = idx.saturating_sub(1);
                self.table_refs
                    .get(idx0)
                    .cloned()
                    .ok_or(SerializeError::InvalidTableRef(idx))
            }
            30 => {
                // TABLEREF_16 (1-based index in Lua)
                let idx = self.read_int(2)? as usize;
                let idx0 = idx.saturating_sub(1);
                self.table_refs
                    .get(idx0)
                    .cloned()
                    .ok_or(SerializeError::InvalidTableRef(idx))
            }
            31 => {
                // TABLEREF_24 (1-based index in Lua)
                let idx = self.read_int(3)? as usize;
                let idx0 = idx.saturating_sub(1);
                self.table_refs
                    .get(idx0)
                    .cloned()
                    .ok_or(SerializeError::InvalidTableRef(idx))
            }
            _ => Err(SerializeError::InvalidTypeByte(idx * 8)),
        }
    }

    fn read_string(&mut self, len: usize) -> Result<LuaValue, SerializeError> {
        let bytes = self.read_bytes(len)?;
        let s = String::from_utf8_lossy(bytes).to_string();
        if len > 2 {
            self.string_refs.push(s.clone());
        }
        Ok(LuaValue::String(s))
    }

    fn read_array(&mut self, len: usize) -> Result<LuaValue, SerializeError> {
        let mut arr = Vec::with_capacity(len);
        for _ in 0..len {
            arr.push(self.read_value()?);
        }
        let result = LuaValue::Array(arr);
        self.table_refs.push(result.clone());
        Ok(result)
    }

    fn read_table(&mut self, len: usize) -> Result<LuaValue, SerializeError> {
        let mut pairs = Vec::with_capacity(len);
        for _ in 0..len {
            let key = self.read_value()?;
            let value = self.read_value()?;
            pairs.push((key, value));
        }
        let result = LuaValue::Table(pairs);
        self.table_refs.push(result.clone());
        Ok(result)
    }

    fn read_mixed(&mut self, arr_len: usize, hash_len: usize) -> Result<LuaValue, SerializeError> {
        let mut pairs = Vec::with_capacity(arr_len + hash_len);

        // Array part (implicit 1-based keys)
        for i in 1..=arr_len {
            let value = self.read_value()?;
            pairs.push((LuaValue::Int(i as i64), value));
        }

        // Hash part
        for _ in 0..hash_len {
            let key = self.read_value()?;
            let value = self.read_value()?;
            pairs.push((key, value));
        }

        let result = LuaValue::Table(pairs);
        self.table_refs.push(result.clone());
        Ok(result)
    }
}

/// Serialize a LuaValue to bytes
pub fn serialize(value: &LuaValue) -> Vec<u8> {
    Serializer::new().serialize(value)
}

/// Deserialize bytes to a LuaValue
pub fn deserialize(data: &[u8]) -> Result<LuaValue, SerializeError> {
    Deserializer::new(data).deserialize()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_small_int() {
        for i in 0..=127i64 {
            let val = LuaValue::Int(i);
            let bytes = serialize(&val);
            let decoded = deserialize(&bytes).unwrap();
            assert_eq!(val, decoded, "Failed for i={}", i);
        }
    }

    #[test]
    fn test_string() {
        let val = LuaValue::String("Hello, World!".to_string());
        let bytes = serialize(&val);
        let decoded = deserialize(&bytes).unwrap();
        assert_eq!(val, decoded);
    }

    #[test]
    fn test_table() {
        let val = LuaValue::Table(vec![
            (LuaValue::String("key".to_string()), LuaValue::Int(42)),
        ]);
        let bytes = serialize(&val);
        let decoded = deserialize(&bytes).unwrap();
        assert_eq!(val, decoded);
    }
}
