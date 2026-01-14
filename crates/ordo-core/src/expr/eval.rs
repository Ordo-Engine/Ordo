//! Expression evaluator
//!
//! Evaluates expression AST against a context

use super::ast::{BinaryOp, Expr, UnaryOp};
use super::functions::FunctionRegistry;
use crate::context::{Context, Value};
use crate::error::{OrdoError, Result};
use std::collections::HashMap;

/// Expression evaluator
pub struct Evaluator {
    /// Function registry
    functions: FunctionRegistry,
}

impl Default for Evaluator {
    fn default() -> Self {
        Self::new()
    }
}

impl Evaluator {
    /// Create a new evaluator
    pub fn new() -> Self {
        Self {
            functions: FunctionRegistry::new(),
        }
    }

    /// Create an evaluator with custom function registry
    pub fn with_functions(functions: FunctionRegistry) -> Self {
        Self { functions }
    }

    /// Get function registry for customization
    pub fn functions_mut(&mut self) -> &mut FunctionRegistry {
        &mut self.functions
    }

    /// Evaluate an expression
    pub fn eval(&self, expr: &Expr, ctx: &Context) -> Result<Value> {
        match expr {
            Expr::Literal(v) => Ok(v.clone()),

            Expr::Field(path) => ctx
                .get(path)
                .cloned()
                .ok_or_else(|| OrdoError::FieldNotFound {
                    field: path.clone(),
                }),

            Expr::Binary { op, left, right } => self.eval_binary(*op, left, right, ctx),

            Expr::Unary { op, operand } => self.eval_unary(*op, operand, ctx),

            Expr::Call { name, args } => {
                let arg_values: Vec<Value> = args
                    .iter()
                    .map(|arg| self.eval(arg, ctx))
                    .collect::<Result<_>>()?;
                self.functions.call(name, &arg_values)
            }

            Expr::Conditional {
                condition,
                then_branch,
                else_branch,
            } => {
                let cond = self.eval(condition, ctx)?;
                if cond.is_truthy() {
                    self.eval(then_branch, ctx)
                } else {
                    self.eval(else_branch, ctx)
                }
            }

            Expr::Array(elements) => {
                let values: Vec<Value> = elements
                    .iter()
                    .map(|e| self.eval(e, ctx))
                    .collect::<Result<_>>()?;
                Ok(Value::array(values))
            }

            Expr::Object(pairs) => {
                let mut map = HashMap::new();
                for (key, value_expr) in pairs {
                    let value = self.eval(value_expr, ctx)?;
                    map.insert(key.clone(), value);
                }
                Ok(Value::object(map))
            }

            Expr::Exists(path) => Ok(Value::bool(ctx.get(path).is_some())),

            Expr::Coalesce(exprs) => {
                for expr in exprs {
                    // Try to evaluate, treating FieldNotFound as null
                    match self.eval(expr, ctx) {
                        Ok(v) if !v.is_null() => return Ok(v),
                        Ok(_) => continue, // null, try next
                        Err(OrdoError::FieldNotFound { .. }) => continue, // field not found, try next
                        Err(e) => return Err(e),                          // other errors propagate
                    }
                }
                Ok(Value::Null)
            }
        }
    }

    /// Evaluate binary operation
    fn eval_binary(&self, op: BinaryOp, left: &Expr, right: &Expr, ctx: &Context) -> Result<Value> {
        // Short-circuit evaluation for logical operators
        if op == BinaryOp::And {
            let left_val = self.eval(left, ctx)?;
            if !left_val.is_truthy() {
                return Ok(Value::bool(false));
            }
            let right_val = self.eval(right, ctx)?;
            return Ok(Value::bool(right_val.is_truthy()));
        }

        if op == BinaryOp::Or {
            let left_val = self.eval(left, ctx)?;
            if left_val.is_truthy() {
                return Ok(Value::bool(true));
            }
            let right_val = self.eval(right, ctx)?;
            return Ok(Value::bool(right_val.is_truthy()));
        }

        // Evaluate both sides
        let left_val = self.eval(left, ctx)?;
        let right_val = self.eval(right, ctx)?;

        match op {
            // Arithmetic
            BinaryOp::Add => self.eval_add(&left_val, &right_val),
            BinaryOp::Sub => self.eval_sub(&left_val, &right_val),
            BinaryOp::Mul => self.eval_mul(&left_val, &right_val),
            BinaryOp::Div => self.eval_div(&left_val, &right_val),
            BinaryOp::Mod => self.eval_mod(&left_val, &right_val),

            // Comparison
            BinaryOp::Eq => Ok(Value::bool(left_val == right_val)),
            BinaryOp::Ne => Ok(Value::bool(left_val != right_val)),
            BinaryOp::Lt => self.eval_compare(&left_val, &right_val, std::cmp::Ordering::Less),
            BinaryOp::Le => self.eval_compare_le(&left_val, &right_val),
            BinaryOp::Gt => self.eval_compare(&left_val, &right_val, std::cmp::Ordering::Greater),
            BinaryOp::Ge => self.eval_compare_ge(&left_val, &right_val),

            // Set operations
            BinaryOp::In => self.eval_in(&left_val, &right_val),
            BinaryOp::NotIn => self
                .eval_in(&left_val, &right_val)
                .map(|v| Value::bool(!v.as_bool().unwrap_or(false))),
            BinaryOp::Contains => self.eval_contains(&left_val, &right_val),

            // Already handled above
            BinaryOp::And | BinaryOp::Or => unreachable!(),
        }
    }

    /// Evaluate unary operation
    fn eval_unary(&self, op: UnaryOp, operand: &Expr, ctx: &Context) -> Result<Value> {
        let val = self.eval(operand, ctx)?;

        match op {
            UnaryOp::Not => Ok(Value::bool(!val.is_truthy())),
            UnaryOp::Neg => match val {
                Value::Int(n) => Ok(Value::int(-n)),
                Value::Float(n) => Ok(Value::float(-n)),
                _ => Err(OrdoError::type_error("number", val.type_name())),
            },
        }
    }

    // ==================== Arithmetic operations ====================

    fn eval_add(&self, left: &Value, right: &Value) -> Result<Value> {
        match (left, right) {
            (Value::Int(a), Value::Int(b)) => a
                .checked_add(*b)
                .map(Value::int)
                .ok_or_else(|| OrdoError::eval_error("Integer overflow in addition")),
            (Value::Float(a), Value::Float(b)) => Ok(Value::float(a + b)),
            (Value::Int(a), Value::Float(b)) => Ok(Value::float(*a as f64 + b)),
            (Value::Float(a), Value::Int(b)) => Ok(Value::float(a + *b as f64)),
            (Value::String(a), Value::String(b)) => Ok(Value::string(format!("{}{}", a, b))),
            _ => Err(OrdoError::eval_error(format!(
                "Cannot add {} and {}",
                left.type_name(),
                right.type_name()
            ))),
        }
    }

    fn eval_sub(&self, left: &Value, right: &Value) -> Result<Value> {
        match (left, right) {
            (Value::Int(a), Value::Int(b)) => a
                .checked_sub(*b)
                .map(Value::int)
                .ok_or_else(|| OrdoError::eval_error("Integer overflow in subtraction")),
            (Value::Float(a), Value::Float(b)) => Ok(Value::float(a - b)),
            (Value::Int(a), Value::Float(b)) => Ok(Value::float(*a as f64 - b)),
            (Value::Float(a), Value::Int(b)) => Ok(Value::float(a - *b as f64)),
            _ => Err(OrdoError::eval_error(format!(
                "Cannot subtract {} and {}",
                left.type_name(),
                right.type_name()
            ))),
        }
    }

    fn eval_mul(&self, left: &Value, right: &Value) -> Result<Value> {
        match (left, right) {
            (Value::Int(a), Value::Int(b)) => a
                .checked_mul(*b)
                .map(Value::int)
                .ok_or_else(|| OrdoError::eval_error("Integer overflow in multiplication")),
            (Value::Float(a), Value::Float(b)) => Ok(Value::float(a * b)),
            (Value::Int(a), Value::Float(b)) => Ok(Value::float(*a as f64 * b)),
            (Value::Float(a), Value::Int(b)) => Ok(Value::float(a * *b as f64)),
            _ => Err(OrdoError::eval_error(format!(
                "Cannot multiply {} and {}",
                left.type_name(),
                right.type_name()
            ))),
        }
    }

    fn eval_div(&self, left: &Value, right: &Value) -> Result<Value> {
        match (left, right) {
            (Value::Int(a), Value::Int(b)) => {
                if *b == 0 {
                    return Err(OrdoError::eval_error("Division by zero"));
                }
                Ok(Value::int(a / b))
            }
            (Value::Float(a), Value::Float(b)) => {
                if *b == 0.0 {
                    return Err(OrdoError::eval_error("Division by zero"));
                }
                Ok(Value::float(a / b))
            }
            (Value::Int(a), Value::Float(b)) => {
                if *b == 0.0 {
                    return Err(OrdoError::eval_error("Division by zero"));
                }
                Ok(Value::float(*a as f64 / b))
            }
            (Value::Float(a), Value::Int(b)) => {
                if *b == 0 {
                    return Err(OrdoError::eval_error("Division by zero"));
                }
                Ok(Value::float(a / *b as f64))
            }
            _ => Err(OrdoError::eval_error(format!(
                "Cannot divide {} and {}",
                left.type_name(),
                right.type_name()
            ))),
        }
    }

    fn eval_mod(&self, left: &Value, right: &Value) -> Result<Value> {
        match (left, right) {
            (Value::Int(a), Value::Int(b)) => {
                if *b == 0 {
                    return Err(OrdoError::eval_error("Modulo by zero"));
                }
                Ok(Value::int(a % b))
            }
            _ => Err(OrdoError::eval_error(format!(
                "Cannot modulo {} and {}",
                left.type_name(),
                right.type_name()
            ))),
        }
    }

    // ==================== Comparison operations ====================

    fn eval_compare(
        &self,
        left: &Value,
        right: &Value,
        expected: std::cmp::Ordering,
    ) -> Result<Value> {
        match left.compare(right) {
            Some(ord) => Ok(Value::bool(ord == expected)),
            None => Err(OrdoError::eval_error(format!(
                "Cannot compare {} and {}",
                left.type_name(),
                right.type_name()
            ))),
        }
    }

    fn eval_compare_le(&self, left: &Value, right: &Value) -> Result<Value> {
        match left.compare(right) {
            Some(ord) => Ok(Value::bool(ord != std::cmp::Ordering::Greater)),
            None => Err(OrdoError::eval_error(format!(
                "Cannot compare {} and {}",
                left.type_name(),
                right.type_name()
            ))),
        }
    }

    fn eval_compare_ge(&self, left: &Value, right: &Value) -> Result<Value> {
        match left.compare(right) {
            Some(ord) => Ok(Value::bool(ord != std::cmp::Ordering::Less)),
            None => Err(OrdoError::eval_error(format!(
                "Cannot compare {} and {}",
                left.type_name(),
                right.type_name()
            ))),
        }
    }

    // ==================== Set operations ====================

    fn eval_in(&self, value: &Value, collection: &Value) -> Result<Value> {
        match collection {
            Value::Array(arr) => Ok(Value::bool(arr.contains(value))),
            Value::String(s) => {
                if let Value::String(v) = value {
                    Ok(Value::bool(s.contains(v.as_ref())))
                } else {
                    Err(OrdoError::eval_error(
                        "'in' with string requires string value",
                    ))
                }
            }
            _ => Err(OrdoError::type_error(
                "array or string",
                collection.type_name(),
            )),
        }
    }

    fn eval_contains(&self, collection: &Value, value: &Value) -> Result<Value> {
        // contains is the reverse of in
        self.eval_in(value, collection)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_ctx(json: &str) -> Context {
        Context::from_json(json).unwrap()
    }

    #[test]
    fn test_eval_literal() {
        let eval = Evaluator::new();
        let ctx = Context::default();

        assert_eq!(eval.eval(&Expr::literal(42), &ctx).unwrap(), Value::int(42));
    }

    #[test]
    fn test_eval_field() {
        let eval = Evaluator::new();
        let ctx = make_ctx(r#"{"age": 25}"#);

        assert_eq!(
            eval.eval(&Expr::field("age"), &ctx).unwrap(),
            Value::int(25)
        );
    }

    #[test]
    fn test_eval_comparison() {
        let eval = Evaluator::new();
        let ctx = make_ctx(r#"{"age": 25}"#);

        let expr = Expr::gt(Expr::field("age"), Expr::literal(18));
        assert_eq!(eval.eval(&expr, &ctx).unwrap(), Value::bool(true));

        let expr = Expr::lt(Expr::field("age"), Expr::literal(18));
        assert_eq!(eval.eval(&expr, &ctx).unwrap(), Value::bool(false));
    }

    #[test]
    fn test_eval_logical() {
        let eval = Evaluator::new();
        let ctx = make_ctx(r#"{"age": 25, "status": "active"}"#);

        let expr = Expr::and(
            Expr::gt(Expr::field("age"), Expr::literal(18)),
            Expr::eq(Expr::field("status"), Expr::literal("active")),
        );
        assert_eq!(eval.eval(&expr, &ctx).unwrap(), Value::bool(true));
    }

    #[test]
    fn test_eval_in() {
        let eval = Evaluator::new();
        let ctx = make_ctx(r#"{"status": "active"}"#);

        let expr = Expr::is_in(
            Expr::field("status"),
            Expr::Array(vec![Expr::literal("active"), Expr::literal("pending")]),
        );
        assert_eq!(eval.eval(&expr, &ctx).unwrap(), Value::bool(true));
    }

    #[test]
    fn test_eval_function() {
        let eval = Evaluator::new();
        let ctx = make_ctx(r#"{"name": "hello"}"#);

        let expr = Expr::call("len", vec![Expr::field("name")]);
        assert_eq!(eval.eval(&expr, &ctx).unwrap(), Value::int(5));
    }

    #[test]
    fn test_eval_conditional() {
        let eval = Evaluator::new();
        let ctx = make_ctx(r#"{"premium": true, "price": 100}"#);

        let expr = Expr::conditional(
            Expr::field("premium"),
            Expr::binary(BinaryOp::Mul, Expr::field("price"), Expr::literal(0.9f64)),
            Expr::field("price"),
        );

        let result = eval.eval(&expr, &ctx).unwrap();
        assert_eq!(result, Value::float(90.0));
    }

    #[test]
    fn test_eval_coalesce() {
        let eval = Evaluator::new();
        let ctx = make_ctx(r#"{"in_appid": "wx123"}"#);

        let expr = Expr::coalesce(vec![Expr::field("appid"), Expr::field("in_appid")]);
        assert_eq!(eval.eval(&expr, &ctx).unwrap(), Value::string("wx123"));
    }

    #[test]
    fn test_eval_exists() {
        let eval = Evaluator::new();
        let ctx = make_ctx(r#"{"user": {"name": "Alice"}}"#);

        assert_eq!(
            eval.eval(&Expr::exists("user.name"), &ctx).unwrap(),
            Value::bool(true)
        );
        assert_eq!(
            eval.eval(&Expr::exists("user.age"), &ctx).unwrap(),
            Value::bool(false)
        );
    }
}
