//! Built-in functions
//!
//! Provides built-in functions for expressions

use crate::context::Value;
use crate::error::{OrdoError, Result};
use std::collections::HashMap;
use std::sync::Arc;

/// Function signature type
pub type FunctionFn = Arc<dyn Fn(&[Value]) -> Result<Value> + Send + Sync>;

/// Function registry
pub struct FunctionRegistry {
    functions: HashMap<String, FunctionFn>,
}

impl Default for FunctionRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl FunctionRegistry {
    /// Create a new function registry with built-in functions
    pub fn new() -> Self {
        let mut registry = Self {
            functions: HashMap::new(),
        };
        registry.register_builtins();
        registry
    }

    /// Register built-in functions
    fn register_builtins(&mut self) {
        // String functions
        self.register("len", |args| {
            require_args("len", args, 1)?;
            match &args[0] {
                Value::String(s) => Ok(Value::int(s.len() as i64)),
                Value::Array(a) => Ok(Value::int(a.len() as i64)),
                Value::Object(o) => Ok(Value::int(o.len() as i64)),
                v => Err(OrdoError::type_error(
                    "string, array, or object",
                    v.type_name(),
                )),
            }
        });

        self.register("upper", |args| {
            require_args("upper", args, 1)?;
            let s = require_string("upper", &args[0])?;
            Ok(Value::string(s.to_uppercase()))
        });

        self.register("lower", |args| {
            require_args("lower", args, 1)?;
            let s = require_string("lower", &args[0])?;
            Ok(Value::string(s.to_lowercase()))
        });

        self.register("trim", |args| {
            require_args("trim", args, 1)?;
            let s = require_string("trim", &args[0])?;
            Ok(Value::string(s.trim()))
        });

        self.register("starts_with", |args| {
            require_args("starts_with", args, 2)?;
            let s = require_string("starts_with", &args[0])?;
            let prefix = require_string("starts_with", &args[1])?;
            Ok(Value::bool(s.starts_with(prefix)))
        });

        self.register("ends_with", |args| {
            require_args("ends_with", args, 2)?;
            let s = require_string("ends_with", &args[0])?;
            let suffix = require_string("ends_with", &args[1])?;
            Ok(Value::bool(s.ends_with(suffix)))
        });

        self.register("contains_str", |args| {
            require_args("contains_str", args, 2)?;
            let s = require_string("contains_str", &args[0])?;
            let sub = require_string("contains_str", &args[1])?;
            Ok(Value::bool(s.contains(sub)))
        });

        self.register("substring", |args| {
            if args.len() < 2 || args.len() > 3 {
                return Err(OrdoError::FunctionArgError {
                    name: "substring".to_string(),
                    message: "expected 2 or 3 arguments".to_string(),
                });
            }
            let s = require_string("substring", &args[0])?;
            let start = require_int("substring", &args[1])? as usize;
            let end = if args.len() == 3 {
                require_int("substring", &args[2])? as usize
            } else {
                s.len()
            };
            let result: String = s.chars().skip(start).take(end - start).collect();
            Ok(Value::string(result))
        });

        // Math functions
        self.register("abs", |args| {
            require_args("abs", args, 1)?;
            match &args[0] {
                Value::Int(n) => Ok(Value::int(n.abs())),
                Value::Float(n) => Ok(Value::float(n.abs())),
                v => Err(OrdoError::type_error("number", v.type_name())),
            }
        });

        self.register("min", |args| {
            if args.is_empty() {
                return Err(OrdoError::FunctionArgError {
                    name: "min".to_string(),
                    message: "expected at least 1 argument".to_string(),
                });
            }
            let mut result = &args[0];
            for arg in &args[1..] {
                if arg.compare(result) == Some(std::cmp::Ordering::Less) {
                    result = arg;
                }
            }
            Ok(result.clone())
        });

        self.register("max", |args| {
            if args.is_empty() {
                return Err(OrdoError::FunctionArgError {
                    name: "max".to_string(),
                    message: "expected at least 1 argument".to_string(),
                });
            }
            let mut result = &args[0];
            for arg in &args[1..] {
                if arg.compare(result) == Some(std::cmp::Ordering::Greater) {
                    result = arg;
                }
            }
            Ok(result.clone())
        });

        self.register("floor", |args| {
            require_args("floor", args, 1)?;
            let n = require_float("floor", &args[0])?;
            Ok(Value::int(n.floor() as i64))
        });

        self.register("ceil", |args| {
            require_args("ceil", args, 1)?;
            let n = require_float("ceil", &args[0])?;
            Ok(Value::int(n.ceil() as i64))
        });

        self.register("round", |args| {
            require_args("round", args, 1)?;
            let n = require_float("round", &args[0])?;
            Ok(Value::int(n.round() as i64))
        });

        // Array functions
        self.register("sum", |args| {
            require_args("sum", args, 1)?;
            let arr = require_array("sum", &args[0])?;
            let mut int_sum: i64 = 0;
            let mut float_sum: f64 = 0.0;
            let mut has_float = false;

            for v in arr {
                match v {
                    Value::Int(n) => int_sum += n,
                    Value::Float(n) => {
                        has_float = true;
                        float_sum += n;
                    }
                    _ => return Err(OrdoError::type_error("number", v.type_name())),
                }
            }

            if has_float {
                Ok(Value::float(int_sum as f64 + float_sum))
            } else {
                Ok(Value::int(int_sum))
            }
        });

        self.register("avg", |args| {
            require_args("avg", args, 1)?;
            let arr = require_array("avg", &args[0])?;
            if arr.is_empty() {
                return Ok(Value::float(0.0));
            }

            let mut sum: f64 = 0.0;
            for v in arr {
                match v {
                    Value::Int(n) => sum += *n as f64,
                    Value::Float(n) => sum += n,
                    _ => return Err(OrdoError::type_error("number", v.type_name())),
                }
            }

            Ok(Value::float(sum / arr.len() as f64))
        });

        self.register("count", |args| {
            require_args("count", args, 1)?;
            let arr = require_array("count", &args[0])?;
            Ok(Value::int(arr.len() as i64))
        });

        self.register("first", |args| {
            require_args("first", args, 1)?;
            let arr = require_array("first", &args[0])?;
            Ok(arr.first().cloned().unwrap_or(Value::Null))
        });

        self.register("last", |args| {
            require_args("last", args, 1)?;
            let arr = require_array("last", &args[0])?;
            Ok(arr.last().cloned().unwrap_or(Value::Null))
        });

        // Type functions
        self.register("type", |args| {
            require_args("type", args, 1)?;
            Ok(Value::string(args[0].type_name()))
        });

        self.register("is_null", |args| {
            require_args("is_null", args, 1)?;
            Ok(Value::bool(args[0].is_null()))
        });

        self.register("is_number", |args| {
            require_args("is_number", args, 1)?;
            Ok(Value::bool(args[0].is_number()))
        });

        self.register("is_string", |args| {
            require_args("is_string", args, 1)?;
            Ok(Value::bool(args[0].is_string()))
        });

        self.register("is_array", |args| {
            require_args("is_array", args, 1)?;
            Ok(Value::bool(args[0].is_array()))
        });

        // Conversion functions
        self.register("to_int", |args| {
            require_args("to_int", args, 1)?;
            match &args[0] {
                Value::Int(n) => Ok(Value::int(*n)),
                Value::Float(n) => Ok(Value::int(*n as i64)),
                Value::String(s) => s
                    .parse::<i64>()
                    .map(Value::int)
                    .map_err(|_| OrdoError::eval_error(format!("Cannot convert '{}' to int", s))),
                Value::Bool(b) => Ok(Value::int(if *b { 1 } else { 0 })),
                v => Err(OrdoError::type_error(
                    "int, float, string, or bool",
                    v.type_name(),
                )),
            }
        });

        self.register("to_float", |args| {
            require_args("to_float", args, 1)?;
            match &args[0] {
                Value::Int(n) => Ok(Value::float(*n as f64)),
                Value::Float(n) => Ok(Value::float(*n)),
                Value::String(s) => s
                    .parse::<f64>()
                    .map(Value::float)
                    .map_err(|_| OrdoError::eval_error(format!("Cannot convert '{}' to float", s))),
                v => Err(OrdoError::type_error(
                    "int, float, or string",
                    v.type_name(),
                )),
            }
        });

        self.register("to_string", |args| {
            require_args("to_string", args, 1)?;
            Ok(Value::string(args[0].to_string()))
        });

        // Date/time functions (basic)
        self.register("now", |_args| {
            Ok(Value::int(chrono::Utc::now().timestamp()))
        });

        self.register("now_millis", |_args| {
            Ok(Value::int(chrono::Utc::now().timestamp_millis()))
        });
    }

    /// Register a custom function
    pub fn register<F>(&mut self, name: impl Into<String>, f: F)
    where
        F: Fn(&[Value]) -> Result<Value> + Send + Sync + 'static,
    {
        self.functions.insert(name.into(), Arc::new(f));
    }

    /// Get a function by name
    pub fn get(&self, name: &str) -> Option<&FunctionFn> {
        self.functions.get(name)
    }

    /// Call a function by name
    pub fn call(&self, name: &str, args: &[Value]) -> Result<Value> {
        let func = self
            .functions
            .get(name)
            .ok_or_else(|| OrdoError::function_not_found(name))?;
        func(args)
    }
}

// ==================== Helper functions ====================

fn require_args(name: &str, args: &[Value], count: usize) -> Result<()> {
    if args.len() != count {
        Err(OrdoError::FunctionArgError {
            name: name.to_string(),
            message: format!("expected {} argument(s), got {}", count, args.len()),
        })
    } else {
        Ok(())
    }
}

fn require_string<'a>(name: &str, value: &'a Value) -> Result<&'a str> {
    value
        .as_str()
        .ok_or_else(|| OrdoError::type_error("string", value.type_name()))
}

fn require_int(name: &str, value: &Value) -> Result<i64> {
    value
        .as_int()
        .ok_or_else(|| OrdoError::type_error("int", value.type_name()))
}

fn require_float(name: &str, value: &Value) -> Result<f64> {
    value
        .as_float()
        .ok_or_else(|| OrdoError::type_error("number", value.type_name()))
}

fn require_array<'a>(name: &str, value: &'a Value) -> Result<&'a Vec<Value>> {
    value
        .as_array()
        .ok_or_else(|| OrdoError::type_error("array", value.type_name()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_len() {
        let registry = FunctionRegistry::new();

        assert_eq!(
            registry.call("len", &[Value::string("hello")]).unwrap(),
            Value::int(5)
        );
        assert_eq!(
            registry
                .call("len", &[Value::array(vec![Value::int(1), Value::int(2)])])
                .unwrap(),
            Value::int(2)
        );
    }

    #[test]
    fn test_string_functions() {
        let registry = FunctionRegistry::new();

        assert_eq!(
            registry.call("upper", &[Value::string("hello")]).unwrap(),
            Value::string("HELLO")
        );
        assert_eq!(
            registry.call("lower", &[Value::string("HELLO")]).unwrap(),
            Value::string("hello")
        );
        assert_eq!(
            registry
                .call("trim", &[Value::string("  hello  ")])
                .unwrap(),
            Value::string("hello")
        );
    }

    #[test]
    fn test_math_functions() {
        let registry = FunctionRegistry::new();

        assert_eq!(
            registry.call("abs", &[Value::int(-5)]).unwrap(),
            Value::int(5)
        );
        assert_eq!(
            registry
                .call("min", &[Value::int(3), Value::int(1), Value::int(2)])
                .unwrap(),
            Value::int(1)
        );
        assert_eq!(
            registry
                .call("max", &[Value::int(3), Value::int(1), Value::int(2)])
                .unwrap(),
            Value::int(3)
        );
    }

    #[test]
    fn test_array_functions() {
        let registry = FunctionRegistry::new();

        let arr = Value::array(vec![Value::int(1), Value::int(2), Value::int(3)]);

        assert_eq!(registry.call("sum", &[arr.clone()]).unwrap(), Value::int(6));
        assert_eq!(
            registry.call("avg", &[arr.clone()]).unwrap(),
            Value::float(2.0)
        );
        assert_eq!(
            registry.call("count", &[arr.clone()]).unwrap(),
            Value::int(3)
        );
        assert_eq!(
            registry.call("first", &[arr.clone()]).unwrap(),
            Value::int(1)
        );
        assert_eq!(registry.call("last", &[arr]).unwrap(), Value::int(3));
    }
}
