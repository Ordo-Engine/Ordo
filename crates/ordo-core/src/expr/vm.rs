//! Stack-based virtual machine for bytecode execution
//!
//! This VM executes compiled bytecode expressions. It uses a stack-based
//! architecture for simplicity and efficiency.

use super::ast::{BinaryOp, UnaryOp};
use super::bytecode::{CompiledExpr, Opcode};
use super::functions::FunctionRegistry;
use crate::context::{Context, Value};
use crate::error::{OrdoError, Result};
use hashbrown::HashMap;

/// Stack-based virtual machine for expression execution
pub struct BytecodeVM {
    /// Function registry for built-in functions
    functions: FunctionRegistry,
    /// Execution stack
    stack: Vec<Value>,
}

impl Default for BytecodeVM {
    fn default() -> Self {
        Self::new()
    }
}

impl BytecodeVM {
    /// Create a new VM
    pub fn new() -> Self {
        Self {
            functions: FunctionRegistry::new(),
            stack: Vec::with_capacity(32), // Pre-allocate reasonable stack size
        }
    }

    /// Create a VM with custom function registry
    pub fn with_functions(functions: FunctionRegistry) -> Self {
        Self {
            functions,
            stack: Vec::with_capacity(32),
        }
    }

    /// Execute a compiled expression against a context
    pub fn execute(&mut self, compiled: &CompiledExpr, ctx: &Context) -> Result<Value> {
        self.stack.clear();
        let mut ip = 0; // Instruction pointer

        while ip < compiled.instructions.len() {
            let instruction = &compiled.instructions[ip];
            ip += 1;

            match instruction {
                Opcode::LoadConst(idx) => {
                    let value = compiled
                        .constants
                        .get(*idx as usize)
                        .ok_or_else(|| OrdoError::eval_error("Invalid constant index"))?
                        .clone();
                    self.stack.push(value);
                }

                Opcode::LoadField(idx) => {
                    let field = compiled
                        .fields
                        .get(*idx as usize)
                        .ok_or_else(|| OrdoError::eval_error("Invalid field index"))?;
                    let value =
                        ctx.get(field)
                            .cloned()
                            .ok_or_else(|| OrdoError::FieldNotFound {
                                field: field.clone(),
                            })?;
                    self.stack.push(value);
                }

                Opcode::BinaryOp(op) => {
                    let right = self.pop()?;
                    let left = self.pop()?;
                    let result = self.eval_binary(*op, &left, &right)?;
                    self.stack.push(result);
                }

                Opcode::UnaryOp(op) => {
                    let operand = self.pop()?;
                    let result = self.eval_unary(*op, &operand)?;
                    self.stack.push(result);
                }

                Opcode::Call(func_idx, arg_count) => {
                    let func_name = compiled
                        .functions
                        .get(*func_idx as usize)
                        .ok_or_else(|| OrdoError::eval_error("Invalid function index"))?;

                    // Pop arguments (they were pushed in order, so we need to reverse)
                    let mut args = Vec::with_capacity(*arg_count as usize);
                    for _ in 0..*arg_count {
                        args.push(self.pop()?);
                    }
                    args.reverse();

                    let result = self.functions.call(func_name, &args)?;
                    self.stack.push(result);
                }

                Opcode::JumpIfFalse(offset) => {
                    let value = self.peek()?;
                    if !value.is_truthy() {
                        ip = ((ip as i16) + offset - 1) as usize;
                    }
                }

                Opcode::JumpIfTrue(offset) => {
                    let value = self.peek()?;
                    if value.is_truthy() {
                        ip = ((ip as i16) + offset - 1) as usize;
                    }
                }

                Opcode::Jump(offset) => {
                    ip = ((ip as i16) + offset - 1) as usize;
                }

                Opcode::Pop => {
                    self.pop()?;
                }

                Opcode::Exists(idx) => {
                    let field = compiled
                        .fields
                        .get(*idx as usize)
                        .ok_or_else(|| OrdoError::eval_error("Invalid field index"))?;
                    let exists = ctx.get(field).is_some();
                    self.stack.push(Value::bool(exists));
                }

                Opcode::MakeArray(count) => {
                    let mut elements = Vec::with_capacity(*count as usize);
                    for _ in 0..*count {
                        elements.push(self.pop()?);
                    }
                    elements.reverse();
                    self.stack.push(Value::array(elements));
                }

                Opcode::MakeObject(count) => {
                    let mut pairs = Vec::with_capacity(*count as usize);
                    for _ in 0..*count {
                        let value = self.pop()?;
                        let key = self.pop()?;
                        let key_str = key
                            .as_str()
                            .ok_or_else(|| OrdoError::eval_error("Object key must be string"))?
                            .to_string();
                        pairs.push((key_str, value));
                    }
                    pairs.reverse();
                    let mut map = HashMap::new();
                    for (k, v) in pairs {
                        map.insert(k.into(), v);
                    }
                    self.stack.push(Value::object_optimized(map));
                }

                Opcode::Dup => {
                    let value = self.peek()?.clone();
                    self.stack.push(value);
                }

                Opcode::Return => {
                    return self.pop();
                }
            }
        }

        // If we reach here without a Return, return the top of stack or Null
        self.stack
            .pop()
            .ok_or_else(|| OrdoError::eval_error("Empty stack at end of execution"))
    }

    /// Pop a value from the stack
    fn pop(&mut self) -> Result<Value> {
        self.stack
            .pop()
            .ok_or_else(|| OrdoError::eval_error("Stack underflow"))
    }

    /// Peek at the top of the stack
    fn peek(&self) -> Result<&Value> {
        self.stack
            .last()
            .ok_or_else(|| OrdoError::eval_error("Stack underflow"))
    }

    /// Evaluate a binary operation
    fn eval_binary(&self, op: BinaryOp, left: &Value, right: &Value) -> Result<Value> {
        match op {
            // Arithmetic
            BinaryOp::Add => self.eval_add(left, right),
            BinaryOp::Sub => self.eval_sub(left, right),
            BinaryOp::Mul => self.eval_mul(left, right),
            BinaryOp::Div => self.eval_div(left, right),
            BinaryOp::Mod => self.eval_mod(left, right),

            // Comparison
            BinaryOp::Eq => Ok(Value::bool(left == right)),
            BinaryOp::Ne => Ok(Value::bool(left != right)),
            BinaryOp::Lt => self.eval_compare(left, right, std::cmp::Ordering::Less),
            BinaryOp::Le => self.eval_compare_le(left, right),
            BinaryOp::Gt => self.eval_compare(left, right, std::cmp::Ordering::Greater),
            BinaryOp::Ge => self.eval_compare_ge(left, right),

            // Logical (handled via short-circuit in bytecode)
            BinaryOp::And => Ok(Value::bool(left.is_truthy() && right.is_truthy())),
            BinaryOp::Or => Ok(Value::bool(left.is_truthy() || right.is_truthy())),

            // Set operations
            BinaryOp::In => self.eval_in(left, right),
            BinaryOp::NotIn => self
                .eval_in(left, right)
                .map(|v| Value::bool(!v.as_bool().unwrap_or(false))),
            BinaryOp::Contains => self.eval_in(right, left),
        }
    }

    /// Evaluate a unary operation
    fn eval_unary(&self, op: UnaryOp, operand: &Value) -> Result<Value> {
        match op {
            UnaryOp::Not => Ok(Value::bool(!operand.is_truthy())),
            UnaryOp::Neg => match operand {
                Value::Int(n) => Ok(Value::int(-n)),
                Value::Float(n) => Ok(Value::float(-n)),
                _ => Err(OrdoError::type_error("number", operand.type_name())),
            },
        }
    }

    // Arithmetic operations
    fn eval_add(&self, left: &Value, right: &Value) -> Result<Value> {
        match (left, right) {
            (Value::Int(a), Value::Int(b)) => a
                .checked_add(*b)
                .map(Value::int)
                .ok_or_else(|| OrdoError::eval_error("Integer overflow")),
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
                .ok_or_else(|| OrdoError::eval_error("Integer overflow")),
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
                .ok_or_else(|| OrdoError::eval_error("Integer overflow")),
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

    // Comparison operations
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

    // Set operations
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
}

#[cfg(test)]
mod tests {
    use super::super::ast::Expr;
    use super::super::compiler::ExprCompiler;
    use super::*;

    fn compile_and_run(expr: &Expr, ctx: &Context) -> Result<Value> {
        let compiled = ExprCompiler::new().compile(expr);
        let mut vm = BytecodeVM::new();
        vm.execute(&compiled, ctx)
    }

    fn make_ctx(json: &str) -> Context {
        Context::from_json(json).unwrap()
    }

    #[test]
    fn test_vm_literal() {
        let expr = Expr::literal(42);
        let ctx = Context::default();
        let result = compile_and_run(&expr, &ctx).unwrap();
        assert_eq!(result, Value::int(42));
    }

    #[test]
    fn test_vm_field() {
        let expr = Expr::field("age");
        let ctx = make_ctx(r#"{"age": 25}"#);
        let result = compile_and_run(&expr, &ctx).unwrap();
        assert_eq!(result, Value::int(25));
    }

    #[test]
    fn test_vm_arithmetic() {
        let ctx = make_ctx(r#"{"a": 10, "b": 3}"#);

        // a + b = 13
        let expr = Expr::binary(BinaryOp::Add, Expr::field("a"), Expr::field("b"));
        assert_eq!(compile_and_run(&expr, &ctx).unwrap(), Value::int(13));

        // a * b = 30
        let expr = Expr::binary(BinaryOp::Mul, Expr::field("a"), Expr::field("b"));
        assert_eq!(compile_and_run(&expr, &ctx).unwrap(), Value::int(30));
    }

    #[test]
    fn test_vm_comparison() {
        let ctx = make_ctx(r#"{"age": 25}"#);

        // age > 18 = true
        let expr = Expr::binary(BinaryOp::Gt, Expr::field("age"), Expr::literal(18));
        assert_eq!(compile_and_run(&expr, &ctx).unwrap(), Value::bool(true));

        // age < 18 = false
        let expr = Expr::binary(BinaryOp::Lt, Expr::field("age"), Expr::literal(18));
        assert_eq!(compile_and_run(&expr, &ctx).unwrap(), Value::bool(false));
    }

    #[test]
    fn test_vm_short_circuit_and() {
        let ctx = make_ctx(r#"{"a": false, "b": true}"#);

        // false && b should short-circuit and not evaluate b
        let expr = Expr::binary(BinaryOp::And, Expr::field("a"), Expr::field("b"));
        let result = compile_and_run(&expr, &ctx).unwrap();
        assert_eq!(result, Value::bool(false));
    }

    #[test]
    fn test_vm_short_circuit_or() {
        let ctx = make_ctx(r#"{"a": true, "b": false}"#);

        // true || b should short-circuit and not evaluate b
        let expr = Expr::binary(BinaryOp::Or, Expr::field("a"), Expr::field("b"));
        let result = compile_and_run(&expr, &ctx).unwrap();
        assert_eq!(result, Value::bool(true));
    }

    #[test]
    fn test_vm_function_call() {
        let ctx = make_ctx(r#"{"name": "hello"}"#);

        let expr = Expr::call("len", vec![Expr::field("name")]);
        assert_eq!(compile_and_run(&expr, &ctx).unwrap(), Value::int(5));

        let expr = Expr::call("upper", vec![Expr::field("name")]);
        assert_eq!(
            compile_and_run(&expr, &ctx).unwrap(),
            Value::string("HELLO")
        );
    }

    #[test]
    fn test_vm_conditional() {
        let ctx = make_ctx(r#"{"premium": true, "price": 100}"#);

        // if premium then price * 0.9 else price
        let expr = Expr::conditional(
            Expr::field("premium"),
            Expr::binary(BinaryOp::Mul, Expr::field("price"), Expr::literal(0.9f64)),
            Expr::field("price"),
        );
        let result = compile_and_run(&expr, &ctx).unwrap();
        assert_eq!(result, Value::float(90.0));
    }

    #[test]
    fn test_vm_array() {
        let ctx = Context::default();

        let expr = Expr::Array(vec![Expr::literal(1), Expr::literal(2), Expr::literal(3)]);
        let result = compile_and_run(&expr, &ctx).unwrap();

        if let Value::Array(arr) = result {
            assert_eq!(arr.len(), 3);
            assert_eq!(arr[0], Value::int(1));
            assert_eq!(arr[1], Value::int(2));
            assert_eq!(arr[2], Value::int(3));
        } else {
            panic!("Expected array");
        }
    }

    #[test]
    fn test_vm_unary() {
        let ctx = make_ctx(r#"{"flag": true, "num": 5}"#);

        // !flag = false
        let expr = Expr::unary(UnaryOp::Not, Expr::field("flag"));
        assert_eq!(compile_and_run(&expr, &ctx).unwrap(), Value::bool(false));

        // -num = -5
        let expr = Expr::unary(UnaryOp::Neg, Expr::field("num"));
        assert_eq!(compile_and_run(&expr, &ctx).unwrap(), Value::int(-5));
    }

    #[test]
    fn test_vm_in_operator() {
        let ctx = make_ctx(r#"{"status": "active"}"#);

        let expr = Expr::binary(
            BinaryOp::In,
            Expr::field("status"),
            Expr::Array(vec![Expr::literal("active"), Expr::literal("pending")]),
        );
        assert_eq!(compile_and_run(&expr, &ctx).unwrap(), Value::bool(true));
    }

    #[test]
    fn test_vm_exists() {
        let ctx = make_ctx(r#"{"user": {"name": "Alice"}}"#);

        let expr = Expr::exists("user.name");
        assert_eq!(compile_and_run(&expr, &ctx).unwrap(), Value::bool(true));

        let expr = Expr::exists("user.age");
        assert_eq!(compile_and_run(&expr, &ctx).unwrap(), Value::bool(false));
    }

    #[test]
    fn test_vm_complex_expression() {
        let ctx = make_ctx(r#"{"age": 25, "status": "active", "amount": 1500}"#);

        // (age > 18 && status == "active") || amount > 2000
        let expr = Expr::binary(
            BinaryOp::Or,
            Expr::binary(
                BinaryOp::And,
                Expr::binary(BinaryOp::Gt, Expr::field("age"), Expr::literal(18)),
                Expr::binary(BinaryOp::Eq, Expr::field("status"), Expr::literal("active")),
            ),
            Expr::binary(BinaryOp::Gt, Expr::field("amount"), Expr::literal(2000)),
        );
        assert_eq!(compile_and_run(&expr, &ctx).unwrap(), Value::bool(true));
    }
}
