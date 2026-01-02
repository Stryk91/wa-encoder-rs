//! WeakAura type definitions
//!
//! These structs represent the structure of WeakAura data.
//! The main container is a table with a "d" key containing the aura definition.

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Root WeakAura container
/// Contains either a single aura (d) or a group with children (c)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WeakAuraRoot {
    /// The main aura definition
    pub d: WeakAura,
    /// Child auras (for groups)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub c: Option<Vec<WeakAura>>,
}

/// A single WeakAura definition
/// Using Value for flexibility - WA has many optional/dynamic fields
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WeakAura {
    /// Aura identifier/name
    pub id: String,
    /// Unique ID
    #[serde(skip_serializing_if = "Option::is_none")]
    pub uid: Option<String>,
    /// Internal version number
    #[serde(rename = "internalVersion", skip_serializing_if = "Option::is_none")]
    pub internal_version: Option<i64>,
    /// Region type: icon, aurabar, text, texture, etc.
    #[serde(rename = "regionType")]
    pub region_type: String,
    /// Anchor point
    #[serde(rename = "anchorPoint", skip_serializing_if = "Option::is_none")]
    pub anchor_point: Option<String>,
    /// Self point
    #[serde(rename = "selfPoint", skip_serializing_if = "Option::is_none")]
    pub self_point: Option<String>,
    /// X offset
    #[serde(rename = "xOffset", skip_serializing_if = "Option::is_none")]
    pub x_offset: Option<f64>,
    /// Y offset
    #[serde(rename = "yOffset", skip_serializing_if = "Option::is_none")]
    pub y_offset: Option<f64>,
    /// Width
    #[serde(skip_serializing_if = "Option::is_none")]
    pub width: Option<f64>,
    /// Height
    #[serde(skip_serializing_if = "Option::is_none")]
    pub height: Option<f64>,
    /// Triggers configuration
    #[serde(skip_serializing_if = "Option::is_none")]
    pub triggers: Option<Value>,
    /// Load conditions
    #[serde(skip_serializing_if = "Option::is_none")]
    pub load: Option<Value>,
    /// Actions (init, start, finish)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub actions: Option<Value>,
    /// Animations
    #[serde(skip_serializing_if = "Option::is_none")]
    pub animation: Option<Value>,
    /// Conditions
    #[serde(skip_serializing_if = "Option::is_none")]
    pub conditions: Option<Value>,
    /// Sub-regions (text, border, etc.)
    #[serde(rename = "subRegions", skip_serializing_if = "Option::is_none")]
    pub sub_regions: Option<Value>,
    /// Custom text function
    #[serde(rename = "customText", skip_serializing_if = "Option::is_none")]
    pub custom_text: Option<String>,
    /// All other fields (WA has many optional fields)
    #[serde(flatten)]
    pub extra: std::collections::HashMap<String, Value>,
}

/// Lua value representation for serialization
#[derive(Debug, Clone, PartialEq)]
pub enum LuaValue {
    Nil,
    Bool(bool),
    Int(i64),
    Float(f64),
    String(String),
    Array(Vec<LuaValue>),
    Table(Vec<(LuaValue, LuaValue)>),
}

impl LuaValue {
    /// Convert from serde_json Value
    pub fn from_json(value: &Value) -> Self {
        match value {
            Value::Null => LuaValue::Nil,
            Value::Bool(b) => LuaValue::Bool(*b),
            Value::Number(n) => {
                if let Some(i) = n.as_i64() {
                    LuaValue::Int(i)
                } else if let Some(f) = n.as_f64() {
                    // Check if it's actually an integer
                    if f.fract() == 0.0 && f.abs() < i64::MAX as f64 {
                        LuaValue::Int(f as i64)
                    } else {
                        LuaValue::Float(f)
                    }
                } else {
                    LuaValue::Float(0.0)
                }
            }
            Value::String(s) => LuaValue::String(s.clone()),
            Value::Array(arr) => {
                LuaValue::Array(arr.iter().map(LuaValue::from_json).collect())
            }
            Value::Object(obj) => {
                // Check if it's array-like (consecutive integer keys from 1)
                let mut is_array = true;
                let mut max_key = 0i64;
                for key in obj.keys() {
                    if let Ok(i) = key.parse::<i64>() {
                        if i > 0 {
                            max_key = max_key.max(i);
                        } else {
                            is_array = false;
                            break;
                        }
                    } else {
                        is_array = false;
                        break;
                    }
                }

                if is_array && max_key == obj.len() as i64 {
                    // It's an array-like table
                    let mut arr: Vec<(i64, LuaValue)> = obj
                        .iter()
                        .filter_map(|(k, v)| k.parse::<i64>().ok().map(|i| (i, LuaValue::from_json(v))))
                        .collect();
                    arr.sort_by_key(|(k, _)| *k);
                    LuaValue::Array(arr.into_iter().map(|(_, v)| v).collect())
                } else {
                    // It's a general table
                    LuaValue::Table(
                        obj.iter()
                            .map(|(k, v)| {
                                // Try to parse key as number
                                let key = if let Ok(i) = k.parse::<i64>() {
                                    LuaValue::Int(i)
                                } else {
                                    LuaValue::String(k.clone())
                                };
                                (key, LuaValue::from_json(v))
                            })
                            .collect(),
                    )
                }
            }
        }
    }

    /// Convert to serde_json Value
    pub fn to_json(&self) -> Value {
        match self {
            LuaValue::Nil => Value::Null,
            LuaValue::Bool(b) => Value::Bool(*b),
            LuaValue::Int(i) => Value::Number((*i).into()),
            LuaValue::Float(f) => {
                serde_json::Number::from_f64(*f)
                    .map(Value::Number)
                    .unwrap_or(Value::Null)
            }
            LuaValue::String(s) => Value::String(s.clone()),
            LuaValue::Array(arr) => {
                Value::Array(arr.iter().map(|v| v.to_json()).collect())
            }
            LuaValue::Table(pairs) => {
                let mut obj = serde_json::Map::new();
                for (k, v) in pairs {
                    let key = match k {
                        LuaValue::String(s) => s.clone(),
                        LuaValue::Int(i) => i.to_string(),
                        _ => continue, // Skip non-string/int keys
                    };
                    obj.insert(key, v.to_json());
                }
                Value::Object(obj)
            }
        }
    }
}
