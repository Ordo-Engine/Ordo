//! Execution context storage
//!
//! Manages context data during rule execution, including:
//! - Input fact data
//! - Intermediate variables
//! - Execution state

use super::Value;
use std::collections::HashMap;

/// Execution context
///
/// Stores all context data during rule execution
#[derive(Debug, Clone, Default)]
pub struct Context {
    /// Root data (input fact data)
    data: Value,
    /// Variable storage (variables set during execution)
    variables: HashMap<String, Value>,
    /// Current iteration item (used in batch mode)
    current_item: Option<Value>,
    /// Current item index
    current_index: Option<usize>,
}

impl Context {
    /// Create a new context
    pub fn new(data: Value) -> Self {
        Self {
            data,
            variables: HashMap::new(),
            current_item: None,
            current_index: None,
        }
    }

    /// Create context from JSON string
    pub fn from_json(json: &str) -> Result<Self, serde_json::Error> {
        let data: Value = serde_json::from_str(json)?;
        Ok(Self::new(data))
    }

    /// Get root data
    #[inline]
    pub fn data(&self) -> &Value {
        &self.data
    }

    /// Get mutable root data
    #[inline]
    pub fn data_mut(&mut self) -> &mut Value {
        &mut self.data
    }

    /// Get value by path
    ///
    /// Supports the following prefixes:
    /// - No prefix or `data.`: get from root data
    /// - `$`: get from variables
    /// - `item.`: get from current iteration item
    pub fn get(&self, path: &str) -> Option<&Value> {
        if let Some(var_name) = path.strip_prefix('$') {
            // Variable reference
            self.variables.get(var_name)
        } else if let Some(item_path) = path.strip_prefix("item.") {
            // Current iteration item field
            self.current_item.as_ref()?.get_path(item_path)
        } else if path == "item" {
            // Current iteration item itself
            self.current_item.as_ref()
        } else if path == "_index" {
            // Special handling for index
            None // TODO: needs improvement
        } else if let Some(data_path) = path.strip_prefix("data.") {
            // Explicit data prefix
            self.data.get_path(data_path)
        } else {
            // Default: get from data
            self.data.get_path(path)
        }
    }

    /// Get variable value
    #[inline]
    pub fn get_variable(&self, name: &str) -> Option<&Value> {
        self.variables.get(name)
    }

    /// Set variable
    #[inline]
    pub fn set_variable(&mut self, name: impl Into<String>, value: Value) {
        self.variables.insert(name.into(), value);
    }

    /// Remove variable
    #[inline]
    pub fn remove_variable(&mut self, name: &str) -> Option<Value> {
        self.variables.remove(name)
    }

    /// Get all variables
    #[inline]
    pub fn variables(&self) -> &HashMap<String, Value> {
        &self.variables
    }

    /// Set current iteration item
    #[inline]
    pub fn set_current_item(&mut self, item: Value, index: usize) {
        self.current_item = Some(item);
        self.current_index = Some(index);
    }

    /// Clear current iteration item
    #[inline]
    pub fn clear_current_item(&mut self) {
        self.current_item = None;
        self.current_index = None;
    }

    /// Get current iteration item
    #[inline]
    pub fn current_item(&self) -> Option<&Value> {
        self.current_item.as_ref()
    }

    /// Get current iteration index
    #[inline]
    pub fn current_index(&self) -> Option<usize> {
        self.current_index
    }

    /// Merge variables from another context
    pub fn merge_variables(&mut self, other: &Context) {
        for (k, v) in &other.variables {
            self.variables.insert(k.clone(), v.clone());
        }
    }

    /// Create child context (shared data, independent variables)
    pub fn child(&self) -> Self {
        Self {
            data: self.data.clone(),
            variables: self.variables.clone(),
            current_item: self.current_item.clone(),
            current_index: self.current_index,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_context_basic() {
        let json = r#"{"user": {"name": "Alice", "age": 25}}"#;
        let ctx = Context::from_json(json).unwrap();

        assert_eq!(ctx.get("user.name"), Some(&Value::string("Alice")));
        assert_eq!(ctx.get("user.age"), Some(&Value::int(25)));
    }

    #[test]
    fn test_context_variables() {
        let mut ctx = Context::new(Value::Null);

        ctx.set_variable("score", Value::int(100));
        assert_eq!(ctx.get("$score"), Some(&Value::int(100)));

        ctx.remove_variable("score");
        assert_eq!(ctx.get("$score"), None);
    }

    #[test]
    fn test_context_item() {
        let mut ctx = Context::new(Value::Null);

        let item = Value::object({
            let mut m = std::collections::HashMap::new();
            m.insert("type".to_string(), Value::string("card"));
            m.insert("amount".to_string(), Value::int(1000));
            m
        });

        ctx.set_current_item(item, 0);

        assert_eq!(ctx.get("item.type"), Some(&Value::string("card")));
        assert_eq!(ctx.get("item.amount"), Some(&Value::int(1000)));
        assert_eq!(ctx.current_index(), Some(0));
    }
}
