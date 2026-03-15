//! Built-in functions
//!
//! Provides built-in functions for expressions.
//!
//! # Performance Optimization
//!
//! The default built-in functions are stored in a global singleton (`GLOBAL_BUILTIN_REGISTRY`)
//! to avoid repeated registration overhead. Custom functions can still be added per-registry.

use crate::context::Value;
use crate::error::{OrdoError, Result};
use std::borrow::Cow;
use std::collections::HashMap;
use std::sync::{Arc, OnceLock};

/// Function signature type
pub type FunctionFn = Arc<dyn Fn(&[Value]) -> Result<Value> + Send + Sync>;

/// Global singleton for built-in function registry (shared across all evaluators)
static GLOBAL_BUILTIN_REGISTRY: OnceLock<Arc<FunctionRegistry>> = OnceLock::new();

/// Get the global built-in function registry (lazily initialized)
#[inline]
pub fn global_builtin_registry() -> &'static Arc<FunctionRegistry> {
    GLOBAL_BUILTIN_REGISTRY.get_or_init(|| {
        let mut registry = FunctionRegistry {
            functions: HashMap::new(),
            custom_only: false,
        };
        registry.register_builtins();
        Arc::new(registry)
    })
}

/// Function registry
pub struct FunctionRegistry {
    functions: HashMap<String, FunctionFn>,
    /// If true, this registry only contains custom functions (uses global for builtins)
    custom_only: bool,
}

impl Default for FunctionRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl FunctionRegistry {
    /// Create a new function registry.
    ///
    /// This creates a lightweight registry that delegates built-in function
    /// lookups to the global singleton, avoiding repeated registration overhead.
    pub fn new() -> Self {
        Self {
            functions: HashMap::new(),
            custom_only: true, // Use global registry for builtins
        }
    }

    /// Create a new standalone registry with all built-in functions registered locally.
    /// Use this if you need to modify built-in function behavior.
    pub fn new_standalone() -> Self {
        let mut registry = Self {
            functions: HashMap::new(),
            custom_only: false,
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
                    name: "substring".into(),
                    message: "expected 2 or 3 arguments".into(),
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
                Value::Int(n) => n
                    .checked_abs()
                    .map(Value::int)
                    .ok_or_else(|| OrdoError::eval_error("Integer overflow in abs()")),
                Value::Float(n) => Ok(Value::float(n.abs())),
                v => Err(OrdoError::type_error("number", v.type_name())),
            }
        });

        self.register("min", |args| {
            if args.is_empty() {
                return Err(OrdoError::FunctionArgError {
                    name: "min".into(),
                    message: "expected at least 1 argument".into(),
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
                    name: "max".into(),
                    message: "expected at least 1 argument".into(),
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

        // ==================== Extended built-in functions ====================

        // --- Regex functions (3) ---
        self.register("regex_match", |args| {
            require_args("regex_match", args, 2)?;
            let pattern = require_string("regex_match", &args[0])?;
            let s = require_string("regex_match", &args[1])?;
            let re = compile_regex(pattern)?;
            Ok(Value::bool(re.is_match(s)))
        });

        self.register("regex_find", |args| {
            require_args("regex_find", args, 2)?;
            let pattern = require_string("regex_find", &args[0])?;
            let s = require_string("regex_find", &args[1])?;
            let re = compile_regex(pattern)?;
            match re.find(s) {
                Some(m) => Ok(Value::string(m.as_str())),
                None => Ok(Value::Null),
            }
        });

        self.register("regex_replace", |args| {
            require_args("regex_replace", args, 3)?;
            let pattern = require_string("regex_replace", &args[0])?;
            let s = require_string("regex_replace", &args[1])?;
            let replacement = require_string("regex_replace", &args[2])?;
            let re = compile_regex(pattern)?;
            Ok(Value::string(&*re.replace_all(s, replacement)))
        });

        // --- Time/Date functions (6) ---
        self.register("parse_time", |args| {
            require_args("parse_time", args, 2)?;
            let s = require_string("parse_time", &args[0])?;
            let fmt = require_string("parse_time", &args[1])?;
            let dt = chrono::NaiveDateTime::parse_from_str(s, fmt)
                .map_err(|e| OrdoError::eval_error(format!("parse_time: {}", e)))?;
            Ok(Value::int(dt.and_utc().timestamp()))
        });

        self.register("format_time", |args| {
            require_args("format_time", args, 2)?;
            let ts = require_int("format_time", &args[0])?;
            let fmt = require_string("format_time", &args[1])?;
            let dt = chrono::DateTime::from_timestamp(ts, 0)
                .ok_or_else(|| OrdoError::eval_error("format_time: invalid timestamp"))?;
            Ok(Value::string(dt.format(fmt).to_string()))
        });

        self.register("time_diff", |args| {
            require_args("time_diff", args, 3)?;
            let t1 = require_int("time_diff", &args[0])?;
            let t2 = require_int("time_diff", &args[1])?;
            let unit = require_string("time_diff", &args[2])?;
            let diff = t1 - t2;
            let result = match unit {
                "seconds" | "s" => diff,
                "minutes" | "m" => diff / 60,
                "hours" | "h" => diff / 3600,
                "days" | "d" => diff / 86400,
                _ => {
                    return Err(OrdoError::eval_error(format!(
                        "time_diff: unknown unit '{}'",
                        unit
                    )))
                }
            };
            Ok(Value::int(result))
        });

        self.register("date_add", |args| {
            require_args("date_add", args, 3)?;
            let ts = require_int("date_add", &args[0])?;
            let amount = require_int("date_add", &args[1])?;
            let unit = require_string("date_add", &args[2])?;
            let seconds = match unit {
                "seconds" | "s" => amount,
                "minutes" | "m" => amount * 60,
                "hours" | "h" => amount * 3600,
                "days" | "d" => amount * 86400,
                _ => {
                    return Err(OrdoError::eval_error(format!(
                        "date_add: unknown unit '{}'",
                        unit
                    )))
                }
            };
            Ok(Value::int(ts + seconds))
        });

        self.register("time_of_day", |args| {
            require_args("time_of_day", args, 1)?;
            let ts = require_int("time_of_day", &args[0])?;
            let dt = chrono::DateTime::from_timestamp(ts, 0)
                .ok_or_else(|| OrdoError::eval_error("time_of_day: invalid timestamp"))?;
            Ok(Value::string(dt.format("%H:%M:%S").to_string()))
        });

        self.register("day_of_week", |args| {
            require_args("day_of_week", args, 1)?;
            let ts = require_int("day_of_week", &args[0])?;
            let dt = chrono::DateTime::from_timestamp(ts, 0)
                .ok_or_else(|| OrdoError::eval_error("day_of_week: invalid timestamp"))?;
            Ok(Value::int(
                dt.format("%u").to_string().parse::<i64>().unwrap(),
            ))
        });

        // --- String functions (8) ---
        self.register("replace", |args| {
            require_args("replace", args, 3)?;
            let s = require_string("replace", &args[0])?;
            let old = require_string("replace", &args[1])?;
            let new = require_string("replace", &args[2])?;
            Ok(Value::string(s.replace(old, new)))
        });

        self.register("split", |args| {
            require_args("split", args, 2)?;
            let s = require_string("split", &args[0])?;
            let delim = require_string("split", &args[1])?;
            let parts: Vec<Value> = s.split(delim).map(Value::string).collect();
            Ok(Value::array(parts))
        });

        self.register("join", |args| {
            require_args("join", args, 2)?;
            let arr = require_array("join", &args[0])?;
            let delim = require_string("join", &args[1])?;
            let parts: std::result::Result<Vec<&str>, _> = arr
                .iter()
                .map(|v| {
                    v.as_str()
                        .ok_or_else(|| OrdoError::type_error("string", v.type_name()))
                })
                .collect();
            Ok(Value::string(parts?.join(delim)))
        });

        self.register("pad_left", |args| {
            require_args("pad_left", args, 3)?;
            let s = require_string("pad_left", &args[0])?;
            let raw_width = require_int("pad_left", &args[1])?;
            if raw_width < 0 {
                return Ok(Value::string(s));
            }
            let width = raw_width as usize;
            let ch = require_string("pad_left", &args[2])?;
            let pad_char = ch.chars().next().unwrap_or(' ');
            if s.len() >= width {
                Ok(Value::string(s))
            } else {
                let padding: String = std::iter::repeat(pad_char).take(width - s.len()).collect();
                Ok(Value::string(format!("{}{}", padding, s)))
            }
        });

        self.register("pad_right", |args| {
            require_args("pad_right", args, 3)?;
            let s = require_string("pad_right", &args[0])?;
            let raw_width = require_int("pad_right", &args[1])?;
            if raw_width < 0 {
                return Ok(Value::string(s));
            }
            let width = raw_width as usize;
            let ch = require_string("pad_right", &args[2])?;
            let pad_char = ch.chars().next().unwrap_or(' ');
            if s.len() >= width {
                Ok(Value::string(s))
            } else {
                let padding: String = std::iter::repeat(pad_char).take(width - s.len()).collect();
                Ok(Value::string(format!("{}{}", s, padding)))
            }
        });

        self.register("char_at", |args| {
            require_args("char_at", args, 2)?;
            let s = require_string("char_at", &args[0])?;
            let idx = require_int("char_at", &args[1])? as usize;
            match s.chars().nth(idx) {
                Some(c) => Ok(Value::string(c.to_string())),
                None => Ok(Value::Null),
            }
        });

        self.register("index_of", |args| {
            require_args("index_of", args, 2)?;
            let s = require_string("index_of", &args[0])?;
            let sub = require_string("index_of", &args[1])?;
            match s.find(sub) {
                Some(pos) => Ok(Value::int(pos as i64)),
                None => Ok(Value::int(-1)),
            }
        });

        self.register("format", |args| {
            if args.is_empty() {
                return Err(OrdoError::FunctionArgError {
                    name: "format".into(),
                    message: "expected at least 1 argument".into(),
                });
            }
            let template = require_string("format", &args[0])?;
            let mut result = template.to_string();
            for (i, arg) in args[1..].iter().enumerate() {
                let placeholder = format!("{{{}}}", i);
                let value_str = match arg {
                    Value::String(s) => s.to_string(),
                    other => other.to_string(),
                };
                result = result.replace(&placeholder, &value_str);
            }
            Ok(Value::string(result))
        });

        // --- Encoding functions (4) ---
        #[cfg(feature = "extended-functions")]
        {
            self.register("base64_encode", |args| {
                require_args("base64_encode", args, 1)?;
                let s = require_string("base64_encode", &args[0])?;
                use base64::Engine;
                Ok(Value::string(
                    base64::engine::general_purpose::STANDARD.encode(s.as_bytes()),
                ))
            });

            self.register("base64_decode", |args| {
                require_args("base64_decode", args, 1)?;
                let s = require_string("base64_decode", &args[0])?;
                use base64::Engine;
                let bytes = base64::engine::general_purpose::STANDARD
                    .decode(s.as_bytes())
                    .map_err(|e| OrdoError::eval_error(format!("base64_decode: {}", e)))?;
                let decoded = String::from_utf8(bytes).map_err(|e| {
                    OrdoError::eval_error(format!("base64_decode: invalid UTF-8: {}", e))
                })?;
                Ok(Value::string(decoded))
            });
        }

        #[cfg(feature = "extended-functions")]
        self.register("url_encode", |args| {
            require_args("url_encode", args, 1)?;
            let s = require_string("url_encode", &args[0])?;
            Ok(Value::string(&*urlencoding::encode(s)))
        });

        #[cfg(feature = "extended-functions")]
        self.register("url_decode", |args| {
            require_args("url_decode", args, 1)?;
            let s = require_string("url_decode", &args[0])?;
            let decoded = urlencoding::decode(s)
                .map_err(|e| OrdoError::eval_error(format!("url_decode: {}", e)))?;
            Ok(Value::string(&*decoded))
        });

        // --- Crypto functions (3) ---
        #[cfg(feature = "extended-functions")]
        {
            self.register("md5", |args| {
                require_args("md5", args, 1)?;
                let s = require_string("md5", &args[0])?;
                use md5::Digest;
                let hash = md5::Md5::digest(s.as_bytes());
                Ok(Value::string(format!("{:x}", hash)))
            });

            self.register("sha256", |args| {
                require_args("sha256", args, 1)?;
                let s = require_string("sha256", &args[0])?;
                use sha2::Digest;
                let hash = sha2::Sha256::digest(s.as_bytes());
                Ok(Value::string(format!("{:x}", hash)))
            });

            self.register("hmac_sha256", |args| {
                require_args("hmac_sha256", args, 2)?;
                let key = require_string("hmac_sha256", &args[0])?;
                let msg = require_string("hmac_sha256", &args[1])?;
                use hmac::{Hmac, Mac};
                type HmacSha256 = Hmac<sha2::Sha256>;
                let mut mac = HmacSha256::new_from_slice(key.as_bytes())
                    .map_err(|e| OrdoError::eval_error(format!("hmac_sha256: {}", e)))?;
                mac.update(msg.as_bytes());
                let result = mac.finalize();
                let hex: String = result
                    .into_bytes()
                    .iter()
                    .map(|b| format!("{:02x}", b))
                    .collect();
                Ok(Value::string(hex))
            });
        }

        // --- UUID function (1) ---
        #[cfg(feature = "extended-functions")]
        self.register("uuid_v4", |_args| {
            Ok(Value::string(uuid::Uuid::new_v4().to_string()))
        });

        // --- Array functions (8) ---
        self.register("sort", |args| {
            require_args("sort", args, 1)?;
            let arr = require_array("sort", &args[0])?;
            let mut sorted = arr.to_vec();
            sorted.sort_by(|a, b| a.compare(b).unwrap_or(std::cmp::Ordering::Equal));
            Ok(Value::array(sorted))
        });

        self.register("reverse", |args| {
            require_args("reverse", args, 1)?;
            let arr = require_array("reverse", &args[0])?;
            let mut reversed = arr.to_vec();
            reversed.reverse();
            Ok(Value::array(reversed))
        });

        self.register("unique", |args| {
            require_args("unique", args, 1)?;
            let arr = require_array("unique", &args[0])?;
            let mut seen = Vec::with_capacity(arr.len());
            for v in arr {
                if !seen.contains(v) {
                    seen.push(v.clone());
                }
            }
            Ok(Value::array(seen))
        });

        self.register("slice", |args| {
            if args.len() < 2 || args.len() > 3 {
                return Err(OrdoError::FunctionArgError {
                    name: "slice".into(),
                    message: "expected 2 or 3 arguments".into(),
                });
            }
            let arr = require_array("slice", &args[0])?;
            let start = require_int("slice", &args[1])? as usize;
            let end = if args.len() == 3 {
                require_int("slice", &args[2])? as usize
            } else {
                arr.len()
            };
            let start = start.min(arr.len());
            let end = end.min(arr.len()).max(start);
            Ok(Value::array(arr[start..end].to_vec()))
        });

        self.register("range", |args| {
            if args.len() < 2 || args.len() > 3 {
                return Err(OrdoError::FunctionArgError {
                    name: "range".into(),
                    message: "expected 2 or 3 arguments".into(),
                });
            }
            let start = require_int("range", &args[0])?;
            let end = require_int("range", &args[1])?;
            let step = if args.len() == 3 {
                let s = require_int("range", &args[2])?;
                if s == 0 {
                    return Err(OrdoError::eval_error("range: step cannot be zero"));
                }
                s
            } else {
                1
            };
            let mut result = Vec::new();
            let mut i = start;
            if step > 0 {
                while i < end {
                    result.push(Value::int(i));
                    i += step;
                }
            } else {
                while i > end {
                    result.push(Value::int(i));
                    i += step;
                }
            }
            Ok(Value::array(result))
        });

        self.register("map_extract", |args| {
            require_args("map_extract", args, 2)?;
            let arr = require_array("map_extract", &args[0])?;
            let field = require_string("map_extract", &args[1])?;
            let result: Vec<Value> = arr
                .iter()
                .map(|v| v.get_path(field).cloned().unwrap_or(Value::Null))
                .collect();
            Ok(Value::array(result))
        });

        self.register("filter_by", |args| {
            require_args("filter_by", args, 3)?;
            let arr = require_array("filter_by", &args[0])?;
            let field = require_string("filter_by", &args[1])?;
            let val = &args[2];
            let result: Vec<Value> = arr
                .iter()
                .filter(|v| v.get_path(field) == Some(val))
                .cloned()
                .collect();
            Ok(Value::array(result))
        });

        self.register("group_by", |args| {
            require_args("group_by", args, 2)?;
            let arr = require_array("group_by", &args[0])?;
            let field = require_string("group_by", &args[1])?;
            let mut groups: Vec<(String, Vec<Value>)> = Vec::new();
            for v in arr {
                let key = v
                    .get_path(field)
                    .map(|fv| match fv {
                        Value::String(s) => s.to_string(),
                        other => other.to_string(),
                    })
                    .unwrap_or_else(|| "null".to_string());
                if let Some(pos) = groups.iter().position(|(k, _)| k == &key) {
                    groups[pos].1.push(v.clone());
                } else {
                    groups.push((key, vec![v.clone()]));
                }
            }
            let mut map = std::collections::HashMap::new();
            for (k, v) in groups {
                map.insert(k, Value::array(v));
            }
            Ok(Value::object(map))
        });

        // --- Object functions (4) ---
        self.register("keys", |args| {
            require_args("keys", args, 1)?;
            let obj = require_object("keys", &args[0])?;
            let keys: Vec<Value> = obj.keys().map(|k| Value::string(k.as_ref())).collect();
            Ok(Value::array(keys))
        });

        self.register("values", |args| {
            require_args("values", args, 1)?;
            let obj = require_object("values", &args[0])?;
            let vals: Vec<Value> = obj.values().cloned().collect();
            Ok(Value::array(vals))
        });

        self.register("has_key", |args| {
            require_args("has_key", args, 2)?;
            let obj = require_object("has_key", &args[0])?;
            let key = require_string("has_key", &args[1])?;
            Ok(Value::bool(obj.contains_key(key)))
        });

        self.register("merge", |args| {
            require_args("merge", args, 2)?;
            let obj1 = require_object("merge", &args[0])?;
            let obj2 = require_object("merge", &args[1])?;
            let mut merged = obj1.clone();
            for (k, v) in obj2 {
                merged.insert(k.clone(), v.clone());
            }
            Ok(Value::object_optimized(merged))
        });

        // --- Type functions (3) ---
        self.register("is_bool", |args| {
            require_args("is_bool", args, 1)?;
            Ok(Value::bool(matches!(&args[0], Value::Bool(_))))
        });

        self.register("is_object", |args| {
            require_args("is_object", args, 1)?;
            Ok(Value::bool(matches!(&args[0], Value::Object(_))))
        });

        self.register("to_bool", |args| {
            require_args("to_bool", args, 1)?;
            Ok(Value::bool(args[0].is_truthy()))
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
    ///
    /// Uses fast path for common built-in functions to avoid HashMap lookup overhead.
    #[inline]
    pub fn call(&self, name: &str, args: &[Value]) -> Result<Value> {
        // Fast path for most common functions - avoids HashMap lookup entirely
        match name {
            "len" => return Self::builtin_len_static(args),
            "sum" => return Self::builtin_sum_static(args),
            "max" => return Self::builtin_max_static(args),
            "min" => return Self::builtin_min_static(args),
            "abs" => return Self::builtin_abs_static(args),
            "count" => return Self::builtin_count_static(args),
            "is_null" => return Self::builtin_is_null_static(args),
            _ => {}
        }

        // Check custom functions first (local registry)
        if let Some(func) = self.functions.get(name) {
            return func(args);
        }

        // Fall back to global registry for other built-in functions
        if self.custom_only {
            if let Some(func) = global_builtin_registry().functions.get(name) {
                return func(args);
            }
        }

        Err(OrdoError::function_not_found(name.to_string()))
    }

    // ==================== Fast path implementations (static methods) ====================
    // These are static methods to avoid any self reference overhead in the hot path

    #[inline]
    fn builtin_len_static(args: &[Value]) -> Result<Value> {
        if args.len() != 1 {
            return Err(OrdoError::FunctionArgError {
                name: "len".into(),
                message: Cow::Owned(format!("expected 1 argument(s), got {}", args.len())),
            });
        }
        match &args[0] {
            Value::String(s) => Ok(Value::int(s.len() as i64)),
            Value::Array(a) => Ok(Value::int(a.len() as i64)),
            Value::Object(o) => Ok(Value::int(o.len() as i64)),
            v => Err(OrdoError::type_error(
                "string, array, or object",
                v.type_name(),
            )),
        }
    }

    #[inline]
    fn builtin_sum_static(args: &[Value]) -> Result<Value> {
        if args.len() != 1 {
            return Err(OrdoError::FunctionArgError {
                name: "sum".into(),
                message: Cow::Owned(format!("expected 1 argument(s), got {}", args.len())),
            });
        }
        let arr = args[0]
            .as_array()
            .ok_or_else(|| OrdoError::type_error("array", args[0].type_name()))?;

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
    }

    #[inline]
    fn builtin_max_static(args: &[Value]) -> Result<Value> {
        if args.is_empty() {
            return Err(OrdoError::FunctionArgError {
                name: "max".into(),
                message: "expected at least 1 argument".into(),
            });
        }
        let mut result = &args[0];
        for arg in &args[1..] {
            if arg.compare(result) == Some(std::cmp::Ordering::Greater) {
                result = arg;
            }
        }
        Ok(result.clone())
    }

    #[inline]
    fn builtin_min_static(args: &[Value]) -> Result<Value> {
        if args.is_empty() {
            return Err(OrdoError::FunctionArgError {
                name: "min".into(),
                message: "expected at least 1 argument".into(),
            });
        }
        let mut result = &args[0];
        for arg in &args[1..] {
            if arg.compare(result) == Some(std::cmp::Ordering::Less) {
                result = arg;
            }
        }
        Ok(result.clone())
    }

    #[inline]
    fn builtin_abs_static(args: &[Value]) -> Result<Value> {
        if args.len() != 1 {
            return Err(OrdoError::FunctionArgError {
                name: "abs".into(),
                message: Cow::Owned(format!("expected 1 argument(s), got {}", args.len())),
            });
        }
        match &args[0] {
            Value::Int(n) => n
                .checked_abs()
                .map(Value::int)
                .ok_or_else(|| OrdoError::eval_error("Integer overflow in abs()")),
            Value::Float(n) => Ok(Value::float(n.abs())),
            v => Err(OrdoError::type_error("number", v.type_name())),
        }
    }

    #[inline]
    fn builtin_count_static(args: &[Value]) -> Result<Value> {
        if args.len() != 1 {
            return Err(OrdoError::FunctionArgError {
                name: "count".into(),
                message: Cow::Owned(format!("expected 1 argument(s), got {}", args.len())),
            });
        }
        let arr = args[0]
            .as_array()
            .ok_or_else(|| OrdoError::type_error("array", args[0].type_name()))?;
        Ok(Value::int(arr.len() as i64))
    }

    #[inline]
    fn builtin_is_null_static(args: &[Value]) -> Result<Value> {
        if args.len() != 1 {
            return Err(OrdoError::FunctionArgError {
                name: "is_null".into(),
                message: Cow::Owned(format!("expected 1 argument(s), got {}", args.len())),
            });
        }
        Ok(Value::bool(args[0].is_null()))
    }
}

// ==================== Helper functions ====================

fn require_args(name: &str, args: &[Value], count: usize) -> Result<()> {
    if args.len() != count {
        Err(OrdoError::FunctionArgError {
            name: Cow::Owned(name.to_string()),
            message: Cow::Owned(format!(
                "expected {} argument(s), got {}",
                count,
                args.len()
            )),
        })
    } else {
        Ok(())
    }
}

fn require_string<'a>(_name: &str, value: &'a Value) -> Result<&'a str> {
    value
        .as_str()
        .ok_or_else(|| OrdoError::type_error("string", value.type_name()))
}

fn require_int(_name: &str, value: &Value) -> Result<i64> {
    value
        .as_int()
        .ok_or_else(|| OrdoError::type_error("int", value.type_name()))
}

fn require_float(_name: &str, value: &Value) -> Result<f64> {
    value
        .as_float()
        .ok_or_else(|| OrdoError::type_error("number", value.type_name()))
}

fn require_array<'a>(_name: &str, value: &'a Value) -> Result<&'a [Value]> {
    value
        .as_array()
        .map(|v| v.as_slice())
        .ok_or_else(|| OrdoError::type_error("array", value.type_name()))
}

fn require_object<'a>(
    _name: &str,
    value: &'a Value,
) -> Result<&'a hashbrown::HashMap<crate::context::IString, Value>> {
    match value {
        Value::Object(map) => Ok(map),
        v => Err(OrdoError::type_error("object", v.type_name())),
    }
}

/// Thread-local LRU cache for compiled regex patterns
fn compile_regex(pattern: &str) -> Result<regex::Regex> {
    use std::cell::RefCell;

    thread_local! {
        static REGEX_CACHE: RefCell<lru::LruCache<String, regex::Regex>> =
            RefCell::new(lru::LruCache::new(std::num::NonZeroUsize::new(64).unwrap()));
    }

    REGEX_CACHE.with(|cache| {
        let mut cache = cache.borrow_mut();
        if let Some(re) = cache.get(pattern) {
            return Ok(re.clone());
        }
        let re = regex::Regex::new(pattern)
            .map_err(|e| OrdoError::eval_error(format!("invalid regex '{}': {}", pattern, e)))?;
        cache.put(pattern.to_string(), re.clone());
        Ok(re)
    })
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

        assert_eq!(
            registry.call("sum", std::slice::from_ref(&arr)).unwrap(),
            Value::int(6)
        );
        assert_eq!(
            registry.call("avg", std::slice::from_ref(&arr)).unwrap(),
            Value::float(2.0)
        );
        assert_eq!(
            registry.call("count", std::slice::from_ref(&arr)).unwrap(),
            Value::int(3)
        );
        assert_eq!(
            registry.call("first", std::slice::from_ref(&arr)).unwrap(),
            Value::int(1)
        );
        assert_eq!(
            registry.call("last", std::slice::from_ref(&arr)).unwrap(),
            Value::int(3)
        );
    }

    // ==================== Extended function tests ====================

    #[test]
    fn test_regex_functions() {
        let registry = FunctionRegistry::new();

        // regex_match
        assert_eq!(
            registry
                .call(
                    "regex_match",
                    &[Value::string(r"^\d+$"), Value::string("123")]
                )
                .unwrap(),
            Value::bool(true)
        );
        assert_eq!(
            registry
                .call(
                    "regex_match",
                    &[Value::string(r"^\d+$"), Value::string("abc")]
                )
                .unwrap(),
            Value::bool(false)
        );

        // regex_find
        assert_eq!(
            registry
                .call(
                    "regex_find",
                    &[Value::string(r"\d+"), Value::string("abc123def")]
                )
                .unwrap(),
            Value::string("123")
        );
        assert_eq!(
            registry
                .call("regex_find", &[Value::string(r"\d+"), Value::string("abc")])
                .unwrap(),
            Value::Null
        );

        // regex_replace
        assert_eq!(
            registry
                .call(
                    "regex_replace",
                    &[
                        Value::string(r"\d+"),
                        Value::string("a1b2c3"),
                        Value::string("X")
                    ]
                )
                .unwrap(),
            Value::string("aXbXcX")
        );

        // invalid regex
        assert!(registry
            .call(
                "regex_match",
                &[Value::string("[invalid"), Value::string("test")]
            )
            .is_err());
    }

    #[test]
    fn test_time_functions() {
        let registry = FunctionRegistry::new();

        // parse_time + format_time round-trip
        let ts = registry
            .call(
                "parse_time",
                &[
                    Value::string("2024-01-15 10:30:00"),
                    Value::string("%Y-%m-%d %H:%M:%S"),
                ],
            )
            .unwrap();
        let formatted = registry
            .call("format_time", &[ts.clone(), Value::string("%Y-%m-%d")])
            .unwrap();
        assert_eq!(formatted, Value::string("2024-01-15"));

        // time_diff
        let t1 = Value::int(1000);
        let t2 = Value::int(400);
        assert_eq!(
            registry
                .call(
                    "time_diff",
                    &[t1.clone(), t2.clone(), Value::string("seconds")]
                )
                .unwrap(),
            Value::int(600)
        );

        // date_add
        assert_eq!(
            registry
                .call(
                    "date_add",
                    &[Value::int(1000), Value::int(1), Value::string("hours")]
                )
                .unwrap(),
            Value::int(4600)
        );

        // time_of_day
        let tod = registry.call("time_of_day", &[Value::int(0)]).unwrap();
        assert_eq!(tod, Value::string("00:00:00"));

        // day_of_week (1970-01-01 was Thursday = 4)
        assert_eq!(
            registry.call("day_of_week", &[Value::int(0)]).unwrap(),
            Value::int(4)
        );

        // parse_time invalid format
        assert!(registry
            .call(
                "parse_time",
                &[Value::string("not-a-date"), Value::string("%Y-%m-%d")]
            )
            .is_err());
    }

    #[test]
    fn test_extended_string_functions() {
        let registry = FunctionRegistry::new();

        // replace
        assert_eq!(
            registry
                .call(
                    "replace",
                    &[
                        Value::string("hello world"),
                        Value::string("world"),
                        Value::string("rust")
                    ]
                )
                .unwrap(),
            Value::string("hello rust")
        );

        // split
        assert_eq!(
            registry
                .call("split", &[Value::string("a,b,c"), Value::string(",")])
                .unwrap(),
            Value::array(vec![
                Value::string("a"),
                Value::string("b"),
                Value::string("c")
            ])
        );

        // join
        let arr = Value::array(vec![
            Value::string("a"),
            Value::string("b"),
            Value::string("c"),
        ]);
        assert_eq!(
            registry.call("join", &[arr, Value::string("-")]).unwrap(),
            Value::string("a-b-c")
        );

        // pad_left
        assert_eq!(
            registry
                .call(
                    "pad_left",
                    &[Value::string("42"), Value::int(5), Value::string("0")]
                )
                .unwrap(),
            Value::string("00042")
        );

        // pad_right
        assert_eq!(
            registry
                .call(
                    "pad_right",
                    &[Value::string("hi"), Value::int(5), Value::string(".")]
                )
                .unwrap(),
            Value::string("hi...")
        );

        // char_at
        assert_eq!(
            registry
                .call("char_at", &[Value::string("hello"), Value::int(1)])
                .unwrap(),
            Value::string("e")
        );
        assert_eq!(
            registry
                .call("char_at", &[Value::string("hello"), Value::int(99)])
                .unwrap(),
            Value::Null
        );

        // index_of
        assert_eq!(
            registry
                .call("index_of", &[Value::string("hello"), Value::string("ll")])
                .unwrap(),
            Value::int(2)
        );
        assert_eq!(
            registry
                .call("index_of", &[Value::string("hello"), Value::string("xyz")])
                .unwrap(),
            Value::int(-1)
        );

        // format
        assert_eq!(
            registry
                .call(
                    "format",
                    &[
                        Value::string("Hello {0}, you are {1}"),
                        Value::string("Alice"),
                        Value::int(25)
                    ]
                )
                .unwrap(),
            Value::string("Hello Alice, you are 25")
        );
    }

    #[cfg(feature = "extended-functions")]
    #[test]
    fn test_encoding_functions() {
        let registry = FunctionRegistry::new();

        // base64 round-trip
        let encoded = registry
            .call("base64_encode", &[Value::string("hello world")])
            .unwrap();
        assert_eq!(encoded, Value::string("aGVsbG8gd29ybGQ="));
        let decoded = registry.call("base64_decode", &[encoded]).unwrap();
        assert_eq!(decoded, Value::string("hello world"));

        // invalid base64
        assert!(registry
            .call("base64_decode", &[Value::string("not-valid-base64!!!")])
            .is_err());
    }

    #[cfg(feature = "extended-functions")]
    #[test]
    fn test_url_encoding_functions() {
        let registry = FunctionRegistry::new();

        let encoded = registry
            .call("url_encode", &[Value::string("hello world&foo=bar")])
            .unwrap();
        assert_eq!(encoded, Value::string("hello%20world%26foo%3Dbar"));

        let decoded = registry.call("url_decode", &[encoded]).unwrap();
        assert_eq!(decoded, Value::string("hello world&foo=bar"));
    }

    #[cfg(feature = "extended-functions")]
    #[test]
    fn test_crypto_functions() {
        let registry = FunctionRegistry::new();

        // md5
        let hash = registry.call("md5", &[Value::string("hello")]).unwrap();
        assert_eq!(hash, Value::string("5d41402abc4b2a76b9719d911017c592"));

        // sha256
        let hash = registry.call("sha256", &[Value::string("hello")]).unwrap();
        assert_eq!(
            hash,
            Value::string("2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824")
        );

        // hmac_sha256
        let mac = registry
            .call(
                "hmac_sha256",
                &[Value::string("secret"), Value::string("message")],
            )
            .unwrap();
        // just verify it returns a 64-char hex string
        if let Value::String(s) = &mac {
            assert_eq!(s.len(), 64);
        } else {
            panic!("expected string");
        }
    }

    #[cfg(feature = "extended-functions")]
    #[test]
    fn test_uuid_function() {
        let registry = FunctionRegistry::new();
        let uuid1 = registry.call("uuid_v4", &[]).unwrap();
        let uuid2 = registry.call("uuid_v4", &[]).unwrap();
        // UUIDs should be different
        assert_ne!(uuid1, uuid2);
        // Should be valid UUID format (36 chars with hyphens)
        if let Value::String(s) = &uuid1 {
            assert_eq!(s.len(), 36);
        } else {
            panic!("expected string");
        }
    }

    #[test]
    fn test_extended_array_functions() {
        let registry = FunctionRegistry::new();

        // sort
        let arr = Value::array(vec![Value::int(3), Value::int(1), Value::int(2)]);
        assert_eq!(
            registry.call("sort", std::slice::from_ref(&arr)).unwrap(),
            Value::array(vec![Value::int(1), Value::int(2), Value::int(3)])
        );

        // reverse
        assert_eq!(
            registry
                .call("reverse", std::slice::from_ref(&arr))
                .unwrap(),
            Value::array(vec![Value::int(2), Value::int(1), Value::int(3)])
        );

        // unique
        let dup = Value::array(vec![
            Value::int(1),
            Value::int(2),
            Value::int(1),
            Value::int(3),
        ]);
        assert_eq!(
            registry.call("unique", std::slice::from_ref(&dup)).unwrap(),
            Value::array(vec![Value::int(1), Value::int(2), Value::int(3)])
        );

        // slice
        let arr5 = Value::array(vec![
            Value::int(10),
            Value::int(20),
            Value::int(30),
            Value::int(40),
            Value::int(50),
        ]);
        assert_eq!(
            registry
                .call("slice", &[arr5.clone(), Value::int(1), Value::int(3)])
                .unwrap(),
            Value::array(vec![Value::int(20), Value::int(30)])
        );

        // range
        assert_eq!(
            registry
                .call("range", &[Value::int(0), Value::int(5)])
                .unwrap(),
            Value::array(vec![
                Value::int(0),
                Value::int(1),
                Value::int(2),
                Value::int(3),
                Value::int(4)
            ])
        );
        assert_eq!(
            registry
                .call("range", &[Value::int(0), Value::int(10), Value::int(3)])
                .unwrap(),
            Value::array(vec![
                Value::int(0),
                Value::int(3),
                Value::int(6),
                Value::int(9)
            ])
        );
        // range with zero step should error
        assert!(registry
            .call("range", &[Value::int(0), Value::int(5), Value::int(0)])
            .is_err());

        // empty array edge cases
        let empty = Value::array(vec![]);
        assert_eq!(
            registry.call("sort", std::slice::from_ref(&empty)).unwrap(),
            Value::array(vec![])
        );
        assert_eq!(
            registry
                .call("unique", std::slice::from_ref(&empty))
                .unwrap(),
            Value::array(vec![])
        );
    }

    #[test]
    fn test_map_extract_filter_group() {
        let registry = FunctionRegistry::new();

        let users = Value::array(vec![
            Value::object({
                let mut m = std::collections::HashMap::new();
                m.insert("name".to_string(), Value::string("Alice"));
                m.insert("dept".to_string(), Value::string("eng"));
                m
            }),
            Value::object({
                let mut m = std::collections::HashMap::new();
                m.insert("name".to_string(), Value::string("Bob"));
                m.insert("dept".to_string(), Value::string("sales"));
                m
            }),
            Value::object({
                let mut m = std::collections::HashMap::new();
                m.insert("name".to_string(), Value::string("Charlie"));
                m.insert("dept".to_string(), Value::string("eng"));
                m
            }),
        ]);

        // map_extract
        let names = registry
            .call("map_extract", &[users.clone(), Value::string("name")])
            .unwrap();
        assert_eq!(
            names,
            Value::array(vec![
                Value::string("Alice"),
                Value::string("Bob"),
                Value::string("Charlie")
            ])
        );

        // filter_by
        let eng = registry
            .call(
                "filter_by",
                &[users.clone(), Value::string("dept"), Value::string("eng")],
            )
            .unwrap();
        if let Value::Array(arr) = &eng {
            assert_eq!(arr.len(), 2);
        } else {
            panic!("expected array");
        }

        // group_by
        let grouped = registry
            .call("group_by", &[users.clone(), Value::string("dept")])
            .unwrap();
        if let Value::Object(map) = &grouped {
            assert_eq!(map.len(), 2);
            assert!(map.contains_key("eng"));
            assert!(map.contains_key("sales"));
        } else {
            panic!("expected object");
        }
    }

    #[test]
    fn test_object_functions() {
        let registry = FunctionRegistry::new();

        let obj = Value::object({
            let mut m = std::collections::HashMap::new();
            m.insert("a".to_string(), Value::int(1));
            m.insert("b".to_string(), Value::int(2));
            m
        });

        // keys
        let keys = registry.call("keys", std::slice::from_ref(&obj)).unwrap();
        if let Value::Array(arr) = &keys {
            assert_eq!(arr.len(), 2);
        } else {
            panic!("expected array");
        }

        // values
        let vals = registry.call("values", std::slice::from_ref(&obj)).unwrap();
        if let Value::Array(arr) = &vals {
            assert_eq!(arr.len(), 2);
        } else {
            panic!("expected array");
        }

        // has_key
        assert_eq!(
            registry
                .call("has_key", &[obj.clone(), Value::string("a")])
                .unwrap(),
            Value::bool(true)
        );
        assert_eq!(
            registry
                .call("has_key", &[obj.clone(), Value::string("z")])
                .unwrap(),
            Value::bool(false)
        );

        // merge
        let obj2 = Value::object({
            let mut m = std::collections::HashMap::new();
            m.insert("b".to_string(), Value::int(99));
            m.insert("c".to_string(), Value::int(3));
            m
        });
        let merged = registry.call("merge", &[obj.clone(), obj2]).unwrap();
        if let Value::Object(map) = &merged {
            assert_eq!(map.len(), 3);
            assert_eq!(map.get("b"), Some(&Value::int(99))); // obj2 overwrites
            assert_eq!(map.get("c"), Some(&Value::int(3)));
        } else {
            panic!("expected object");
        }
    }

    #[test]
    fn test_type_functions_extended() {
        let registry = FunctionRegistry::new();

        assert_eq!(
            registry.call("is_bool", &[Value::bool(true)]).unwrap(),
            Value::bool(true)
        );
        assert_eq!(
            registry.call("is_bool", &[Value::int(1)]).unwrap(),
            Value::bool(false)
        );
        assert_eq!(
            registry
                .call(
                    "is_object",
                    &[Value::object(std::collections::HashMap::new())]
                )
                .unwrap(),
            Value::bool(true)
        );
        assert_eq!(
            registry.call("is_object", &[Value::int(1)]).unwrap(),
            Value::bool(false)
        );

        // to_bool
        assert_eq!(
            registry.call("to_bool", &[Value::int(1)]).unwrap(),
            Value::bool(true)
        );
        assert_eq!(
            registry.call("to_bool", &[Value::int(0)]).unwrap(),
            Value::bool(false)
        );
        assert_eq!(
            registry.call("to_bool", &[Value::Null]).unwrap(),
            Value::bool(false)
        );
        assert_eq!(
            registry.call("to_bool", &[Value::string("hello")]).unwrap(),
            Value::bool(true)
        );
    }
}
