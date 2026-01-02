//! wa-encoder-rs - Native Rust WeakAura encoder/decoder
//!
//! This library provides encoding and decoding of WeakAura import strings
//! without requiring a Lua runtime.

pub mod decoder;
pub mod encodeforprint;
pub mod encoder;
pub mod libserialize;
pub mod types;

pub use decoder::{decode, decode_to_value, validate, DecodeError, ValidationResult};
pub use encoder::{encode, encode_value, EncodeError};
pub use types::LuaValue;
