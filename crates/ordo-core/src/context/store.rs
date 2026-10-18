//! Execution context storage
//!
//! Manages context data during rule execution, including:
//! - Input fact data
//! - Intermediate variables
//! - Execution state

use super::Value;
use hashbrown::HashMap;

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
    /// - `$`: get from variables, then from an input field with the same `$` name
    /// - `item.`: get from current iteration item
    /// - `_index`: get the current iteration index (if set)
    pub fn get(&self, path: &str) -> Option<&Value> {
        if let Some(var_name) = path.strip_prefix('$') {
            // Variable reference, with optional nested path access.
            let var = if let Some((name, nested)) = var_name.split_once('.') {
                self.variables.get(name).and_then(|v| v.get_path(nested))
            } else {
                self.variables.get(var_name)
            };
            // Fall back to an input field whose key starts with `$`, which is how
            // hosts pass external data (`$data`, see `EXTERNAL_DATA_KEY`).
            var.or_else(|| self.data.get_path(path))
        } else if let (Some(item_path), Some(item)) =
            (path.strip_prefix("item."), self.current_item.as_ref())
        {
            // Current iteration item field
            item.get_path(item_path)
        } else if let (true, Some(item)) = (path == "item", self.current_item.as_ref()) {
            // Current iteration item itself
            Some(item)
        } else if path == "_index" {
            // Special handling for index - backed by an internal variable
            self.variables.get("_index")
        } else if let Some(data_path) = path.strip_prefix("data.") {
            // Explicit data prefix, unless the input has its own `data` field:
            // `data.x` must read that field, not quietly miss it.
            self.data
                .get_path(path)
                .or_else(|| self.data.get_path(data_path))
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
        // Expose the current index via a special `_index` variable so it can be accessed from expressions.
        self.variables
            .insert("_index".to_string(), Value::int(index as i64));
    }

    /// Clear current iteration item
    #[inline]
    pub fn clear_current_item(&mut self) {
        self.current_item = None;
        self.current_index = None;
        // Remove the special index variable when iteration ends.
        self.variables.remove("_index");
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
    fn test_context_nested_variable_paths() {
        let mut ctx = Context::new(Value::Null);
        ctx.set_variable(
            "result",
            Value::object({
                let mut m = std::collections::HashMap::new();
                m.insert(
                    "payload".to_string(),
                    Value::object({
                        let mut nested = std::collections::HashMap::new();
                        nested.insert("score".to_string(), Value::int(7));
                        nested
                    }),
                );
                m
            }),
        );

        assert_eq!(ctx.get("$result.payload.score"), Some(&Value::int(7)));
    }

    #[test]
    fn test_dollar_path_falls_back_to_input_field() {
        let mut ctx = Context::from_json(r#"{"$data": {"limits": {"max": 5}}}"#).unwrap();
        assert_eq!(ctx.get("$data.limits.max"), Some(&Value::int(5)));
        assert_eq!(ctx.get("$data.limits.min"), None);

        // A variable with the same name wins.
        ctx.set_variable("data", Value::object(Default::default()));
        assert_eq!(ctx.get("$data.limits.max"), Some(&Value::int(5)));
        ctx.set_variable(
            "data",
            Value::object({
                let mut m = std::collections::HashMap::new();
                m.insert("limits".to_string(), Value::int(1));
                m
            }),
        );
        assert_eq!(ctx.get("$data.limits"), Some(&Value::int(1)));
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
        // `_index` should be available as a special variable reflecting the current index.
        assert_eq!(ctx.get("_index"), Some(&Value::int(0)));
    }

    #[test]
    fn test_input_fields_named_data_and_item() {
        let ctx =
            Context::from_json(r#"{"x": 1, "data": {"y": 2}, "item": {"sku": "A1"}}"#).unwrap();

        assert_eq!(ctx.get("data.y"), Some(&Value::int(2)));
        assert_eq!(ctx.get("data.x"), Some(&Value::int(1)));
        assert_eq!(ctx.get("item.sku"), Some(&Value::string("A1")));
        assert!(ctx.get("item").is_some());
    }
}
