//! Value type system
//!
//! Defines dynamic value types in the rule engine, supporting:
//! - Primitive types: Null, Bool, Int, Float, String
//! - Composite types: Array, Object
//! - Type conversion and comparison operations

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt;

/// Dynamic value type
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(untagged)]
pub enum Value {
    /// Null value
    #[default]
    Null,
    /// Boolean value
    Bool(bool),
    /// 64-bit signed integer
    Int(i64),
    /// 64-bit floating point number
    Float(f64),
    /// String
    String(String),
    /// Array
    Array(Vec<Value>),
    /// Object/Map
    Object(HashMap<String, Value>),
}

impl Value {
    // ==================== Constructors ====================

    /// Create a null value
    #[inline]
    pub fn null() -> Self {
        Self::Null
    }

    /// Create a boolean value
    #[inline]
    pub fn bool(v: bool) -> Self {
        Self::Bool(v)
    }

    /// Create an integer value
    #[inline]
    pub fn int(v: i64) -> Self {
        Self::Int(v)
    }

    /// Create a float value
    #[inline]
    pub fn float(v: f64) -> Self {
        Self::Float(v)
    }

    /// Create a string value
    #[inline]
    pub fn string(v: impl Into<String>) -> Self {
        Self::String(v.into())
    }

    /// Create an array value
    #[inline]
    pub fn array(v: Vec<Value>) -> Self {
        Self::Array(v)
    }

    /// Create an object value
    #[inline]
    pub fn object(v: HashMap<String, Value>) -> Self {
        Self::Object(v)
    }

    // ==================== Type checks ====================

    /// Check if value is null
    #[inline]
    pub fn is_null(&self) -> bool {
        matches!(self, Self::Null)
    }

    /// Check if value is boolean
    #[inline]
    pub fn is_bool(&self) -> bool {
        matches!(self, Self::Bool(_))
    }

    /// Check if value is integer
    #[inline]
    pub fn is_int(&self) -> bool {
        matches!(self, Self::Int(_))
    }

    /// Check if value is float
    #[inline]
    pub fn is_float(&self) -> bool {
        matches!(self, Self::Float(_))
    }

    /// Check if value is a number (int or float)
    #[inline]
    pub fn is_number(&self) -> bool {
        matches!(self, Self::Int(_) | Self::Float(_))
    }

    /// Check if value is string
    #[inline]
    pub fn is_string(&self) -> bool {
        matches!(self, Self::String(_))
    }

    /// Check if value is array
    #[inline]
    pub fn is_array(&self) -> bool {
        matches!(self, Self::Array(_))
    }

    /// Check if value is object
    #[inline]
    pub fn is_object(&self) -> bool {
        matches!(self, Self::Object(_))
    }

    /// Get the type name
    pub fn type_name(&self) -> &'static str {
        match self {
            Self::Null => "null",
            Self::Bool(_) => "bool",
            Self::Int(_) => "int",
            Self::Float(_) => "float",
            Self::String(_) => "string",
            Self::Array(_) => "array",
            Self::Object(_) => "object",
        }
    }

    // ==================== Type conversion ====================

    /// Convert to boolean
    pub fn as_bool(&self) -> Option<bool> {
        match self {
            Self::Bool(v) => Some(*v),
            _ => None,
        }
    }

    /// Convert to integer
    pub fn as_int(&self) -> Option<i64> {
        match self {
            Self::Int(v) => Some(*v),
            Self::Float(v) => Some(*v as i64),
            _ => None,
        }
    }

    /// Convert to float
    pub fn as_float(&self) -> Option<f64> {
        match self {
            Self::Float(v) => Some(*v),
            Self::Int(v) => Some(*v as f64),
            _ => None,
        }
    }

    /// Convert to string reference
    pub fn as_str(&self) -> Option<&str> {
        match self {
            Self::String(v) => Some(v),
            _ => None,
        }
    }

    /// Convert to array reference
    pub fn as_array(&self) -> Option<&Vec<Value>> {
        match self {
            Self::Array(v) => Some(v),
            _ => None,
        }
    }

    /// Convert to mutable array reference
    pub fn as_array_mut(&mut self) -> Option<&mut Vec<Value>> {
        match self {
            Self::Array(v) => Some(v),
            _ => None,
        }
    }

    /// Convert to object reference
    pub fn as_object(&self) -> Option<&HashMap<String, Value>> {
        match self {
            Self::Object(v) => Some(v),
            _ => None,
        }
    }

    /// Convert to mutable object reference
    pub fn as_object_mut(&mut self) -> Option<&mut HashMap<String, Value>> {
        match self {
            Self::Object(v) => Some(v),
            _ => None,
        }
    }

    // ==================== Field access ====================

    /// Get field value by path
    ///
    /// Supports dot-separated paths like "user.profile.name"
    /// Supports array indices like "items.0.price"
    pub fn get_path(&self, path: &str) -> Option<&Value> {
        let parts: Vec<&str> = path.split('.').collect();
        self.get_path_parts(&parts)
    }

    /// Get field value by path parts
    fn get_path_parts(&self, parts: &[&str]) -> Option<&Value> {
        if parts.is_empty() {
            return Some(self);
        }

        let key = parts[0];
        let rest = &parts[1..];

        match self {
            Self::Object(map) => map.get(key).and_then(|v| v.get_path_parts(rest)),
            Self::Array(arr) => {
                // Try to parse as array index
                key.parse::<usize>()
                    .ok()
                    .and_then(|idx| arr.get(idx))
                    .and_then(|v| v.get_path_parts(rest))
            }
            _ => None,
        }
    }

    /// Set value at path (if path exists)
    pub fn set_path(&mut self, path: &str, value: Value) -> bool {
        let parts: Vec<&str> = path.split('.').collect();
        self.set_path_parts(&parts, value)
    }

    fn set_path_parts(&mut self, parts: &[&str], value: Value) -> bool {
        if parts.is_empty() {
            return false;
        }

        if parts.len() == 1 {
            if let Self::Object(map) = self {
                map.insert(parts[0].to_string(), value);
                return true;
            }
            return false;
        }

        let key = parts[0];
        let rest = &parts[1..];

        match self {
            Self::Object(map) => {
                if let Some(child) = map.get_mut(key) {
                    child.set_path_parts(rest, value)
                } else {
                    false
                }
            }
            Self::Array(arr) => {
                if let Ok(idx) = key.parse::<usize>() {
                    if let Some(child) = arr.get_mut(idx) {
                        child.set_path_parts(rest, value)
                    } else {
                        false
                    }
                } else {
                    false
                }
            }
            _ => false,
        }
    }

    // ==================== Truthiness ====================

    /// Convert to boolean (truthy/falsy)
    ///
    /// - Null: false
    /// - Bool: original value
    /// - Int: non-zero is true
    /// - Float: non-zero is true
    /// - String: non-empty is true
    /// - Array: non-empty is true
    /// - Object: non-empty is true
    pub fn is_truthy(&self) -> bool {
        match self {
            Self::Null => false,
            Self::Bool(v) => *v,
            Self::Int(v) => *v != 0,
            Self::Float(v) => *v != 0.0,
            Self::String(v) => !v.is_empty(),
            Self::Array(v) => !v.is_empty(),
            Self::Object(v) => !v.is_empty(),
        }
    }

    // ==================== Comparison ====================

    /// Numeric comparison
    ///
    /// Returns Ordering, or None if comparison is not possible
    pub fn compare(&self, other: &Value) -> Option<std::cmp::Ordering> {
        match (self, other) {
            (Self::Int(a), Self::Int(b)) => Some(a.cmp(b)),
            (Self::Float(a), Self::Float(b)) => a.partial_cmp(b),
            (Self::Int(a), Self::Float(b)) => (*a as f64).partial_cmp(b),
            (Self::Float(a), Self::Int(b)) => a.partial_cmp(&(*b as f64)),
            (Self::String(a), Self::String(b)) => Some(a.cmp(b)),
            (Self::Bool(a), Self::Bool(b)) => Some(a.cmp(b)),
            _ => None,
        }
    }
}

// ==================== From implementations ====================

impl From<bool> for Value {
    fn from(v: bool) -> Self {
        Self::Bool(v)
    }
}

impl From<i32> for Value {
    fn from(v: i32) -> Self {
        Self::Int(v as i64)
    }
}

impl From<i64> for Value {
    fn from(v: i64) -> Self {
        Self::Int(v)
    }
}

impl From<f64> for Value {
    fn from(v: f64) -> Self {
        Self::Float(v)
    }
}

impl From<&str> for Value {
    fn from(v: &str) -> Self {
        Self::String(v.to_string())
    }
}

impl From<String> for Value {
    fn from(v: String) -> Self {
        Self::String(v)
    }
}

impl<T: Into<Value>> From<Vec<T>> for Value {
    fn from(v: Vec<T>) -> Self {
        Self::Array(v.into_iter().map(Into::into).collect())
    }
}

impl<T: Into<Value>> From<Option<T>> for Value {
    fn from(v: Option<T>) -> Self {
        match v {
            Some(v) => v.into(),
            None => Self::Null,
        }
    }
}

// ==================== Display implementation ====================

impl fmt::Display for Value {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Null => write!(f, "null"),
            Self::Bool(v) => write!(f, "{}", v),
            Self::Int(v) => write!(f, "{}", v),
            Self::Float(v) => write!(f, "{}", v),
            Self::String(v) => write!(f, "\"{}\"", v),
            Self::Array(v) => {
                write!(f, "[")?;
                for (i, item) in v.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{}", item)?;
                }
                write!(f, "]")
            }
            Self::Object(v) => {
                write!(f, "{{")?;
                for (i, (k, v)) in v.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "\"{}\": {}", k, v)?;
                }
                write!(f, "}}")
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_value_types() {
        assert!(Value::null().is_null());
        assert!(Value::bool(true).is_bool());
        assert!(Value::int(42).is_int());
        assert!(Value::float(3.15).is_float());
        assert!(Value::string("hello").is_string());
        assert!(Value::array(vec![]).is_array());
        assert!(Value::object(HashMap::new()).is_object());
    }

    #[test]
    fn test_value_truthy() {
        assert!(!Value::Null.is_truthy());
        assert!(!Value::Bool(false).is_truthy());
        assert!(Value::Bool(true).is_truthy());
        assert!(!Value::Int(0).is_truthy());
        assert!(Value::Int(1).is_truthy());
        assert!(!Value::String("".to_string()).is_truthy());
        assert!(Value::String("hello".to_string()).is_truthy());
    }

    #[test]
    fn test_value_path() {
        let mut obj = HashMap::new();
        let mut profile = HashMap::new();
        profile.insert("name".to_string(), Value::string("Alice"));
        profile.insert("age".to_string(), Value::int(25));
        obj.insert("user".to_string(), Value::Object(profile));
        obj.insert(
            "items".to_string(),
            Value::array(vec![Value::int(1), Value::int(2), Value::int(3)]),
        );

        let value = Value::Object(obj);

        assert_eq!(value.get_path("user.name"), Some(&Value::string("Alice")));
        assert_eq!(value.get_path("user.age"), Some(&Value::int(25)));
        assert_eq!(value.get_path("items.0"), Some(&Value::int(1)));
        assert_eq!(value.get_path("items.2"), Some(&Value::int(3)));
        assert_eq!(value.get_path("user.unknown"), None);
    }

    #[test]
    fn test_value_compare() {
        use std::cmp::Ordering;

        assert_eq!(Value::int(1).compare(&Value::int(2)), Some(Ordering::Less));
        assert_eq!(
            Value::int(2).compare(&Value::int(1)),
            Some(Ordering::Greater)
        );
        assert_eq!(Value::int(1).compare(&Value::int(1)), Some(Ordering::Equal));

        assert_eq!(
            Value::float(1.0).compare(&Value::int(2)),
            Some(Ordering::Less)
        );
        assert_eq!(
            Value::int(1).compare(&Value::float(0.5)),
            Some(Ordering::Greater)
        );
    }
}
