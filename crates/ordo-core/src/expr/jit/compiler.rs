//! Cranelift JIT Compiler for Expressions
//!
//! Compiles Expr AST to native machine code using Cranelift.
//!
//! # Architecture
//!
//! The JIT compiler generates functions with the signature:
//! `fn(ctx_ptr: *const u8, result_ptr: *mut u8) -> i32`
//!
//! Where:
//! - `ctx_ptr`: Pointer to serialized context data (JSON)
//! - `result_ptr`: Pointer to write the result value
//! - Returns: 0 on success, error code on failure

use crate::context::{Context, Value};
use crate::error::{OrdoError, Result};
use crate::expr::{BinaryOp, Expr, UnaryOp};

use cranelift::prelude::*;
use cranelift_codegen::settings;
use cranelift_jit::{JITBuilder, JITModule};
use cranelift_module::{Linkage, Module};
use std::collections::HashMap;

/// Error codes returned by JIT-compiled functions
#[repr(i32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JITErrorCode {
    Success = 0,
    NullPointer = 1,
    TypeMismatch = 2,
    DivisionByZero = 3,
    FieldNotFound = 4,
    FunctionNotFound = 5,
    InvalidOperation = 6,
    Overflow = 7,
}

/// Result of a JIT compilation
pub struct CompiledFunction {
    /// Function pointer
    pub func_ptr: *const u8,
    /// Source expression hash
    pub expr_hash: u64,
    /// Estimated code size in bytes
    pub code_size: usize,
}

// Safety: The function pointer is valid for the lifetime of JITModule
unsafe impl Send for CompiledFunction {}
unsafe impl Sync for CompiledFunction {}

impl CompiledFunction {
    /// Execute the compiled function with a context
    ///
    /// # Safety
    /// The caller must ensure the context is valid and the result buffer is properly sized.
    pub unsafe fn call(&self, ctx: &Context) -> Result<Value> {
        // Serialize context to JSON for the native function
        let ctx_json = serde_json::to_string(&ctx.data())
            .map_err(|e| OrdoError::eval_error(format!("Failed to serialize context: {}", e)))?;

        // Prepare result buffer (enough for any Value type)
        let mut result_buf = [0u8; 256];

        // Call the JIT function
        let func: extern "C" fn(*const u8, usize, *mut u8) -> i32 =
            std::mem::transmute(self.func_ptr);

        let error_code = func(ctx_json.as_ptr(), ctx_json.len(), result_buf.as_mut_ptr());

        if error_code != 0 {
            return Err(OrdoError::eval_error(format!(
                "JIT execution failed with code: {}",
                error_code
            )));
        }

        // Deserialize result
        // For now, we store the result as a simple f64 for numeric expressions
        let result_f64 = f64::from_le_bytes(result_buf[0..8].try_into().unwrap());

        Ok(Value::Float(result_f64))
    }
}

/// JIT Compiler using Cranelift
pub struct JITCompiler {
    /// Cranelift JIT module
    module: JITModule,
    /// Codegen context (reusable)
    ctx: codegen::Context,
    /// Function builder context (reusable)
    func_ctx: FunctionBuilderContext,
    /// Compiled functions by hash
    functions: HashMap<u64, CompiledFunction>,
    /// Statistics
    stats: JITStats,
}

/// JIT compilation statistics
#[derive(Debug, Default, Clone)]
pub struct JITStats {
    /// Number of successful compilations
    pub successful_compiles: u64,
    /// Number of failed compilations
    pub failed_compiles: u64,
    /// Total compilation time in nanoseconds
    pub total_compile_time_ns: u64,
    /// Total code size generated
    pub total_code_size: usize,
}

impl JITCompiler {
    /// Create a new JIT compiler
    pub fn new() -> Result<Self> {
        let mut flag_builder = settings::builder();
        // Use non-PLT calling convention for portability (works on ARM64)
        flag_builder.set("use_colocated_libcalls", "false").unwrap();
        flag_builder.set("is_pic", "false").unwrap();

        let isa_builder = cranelift_native::builder()
            .map_err(|e| OrdoError::eval_error(format!("Failed to create ISA builder: {}", e)))?;

        let isa = isa_builder
            .finish(settings::Flags::new(flag_builder))
            .map_err(|e| OrdoError::eval_error(format!("Failed to create ISA: {}", e)))?;

        let mut builder = JITBuilder::with_isa(isa, cranelift_module::default_libcall_names());
        // Disable hotswap for simpler memory management
        builder.hotswap(false);

        let module = JITModule::new(builder);
        let ctx = module.make_context();
        let func_ctx = FunctionBuilderContext::new();

        Ok(Self {
            module,
            ctx,
            func_ctx,
            functions: HashMap::new(),
            stats: JITStats::default(),
        })
    }

    /// Get compilation statistics
    pub fn stats(&self) -> &JITStats {
        &self.stats
    }

    /// Check if an expression has been compiled
    pub fn is_compiled(&self, hash: u64) -> bool {
        self.functions.contains_key(&hash)
    }

    /// Get a compiled function by hash
    pub fn get(&self, hash: u64) -> Option<&CompiledFunction> {
        self.functions.get(&hash)
    }

    /// Compile an expression to native code
    pub fn compile(&mut self, expr: &Expr, hash: u64) -> Result<&CompiledFunction> {
        if self.functions.contains_key(&hash) {
            return Ok(self.functions.get(&hash).unwrap());
        }

        let start = std::time::Instant::now();

        // Analyze expression to determine if it's JIT-compilable
        if !Self::is_jit_compilable(expr) {
            self.stats.failed_compiles += 1;
            return Err(OrdoError::eval_error(
                "Expression is not JIT-compilable".to_string(),
            ));
        }

        // Create function signature
        // fn(ctx_ptr: i64, ctx_len: i64, result_ptr: i64) -> i32
        let ptr_type = self.module.target_config().pointer_type();
        let mut sig = self.module.make_signature();
        sig.params.push(AbiParam::new(ptr_type)); // ctx_ptr
        sig.params.push(AbiParam::new(types::I64)); // ctx_len
        sig.params.push(AbiParam::new(ptr_type)); // result_ptr
        sig.returns.push(AbiParam::new(types::I32)); // error code

        // Declare function
        let func_name = format!("jit_expr_{}", hash);
        let func_id = self
            .module
            .declare_function(&func_name, Linkage::Local, &sig)
            .map_err(|e| OrdoError::eval_error(format!("Failed to declare function: {}", e)))?;

        // Build function
        self.ctx.func.signature = sig;
        self.ctx.func.name = cranelift_codegen::ir::UserFuncName::user(0, func_id.as_u32());

        {
            let mut builder = FunctionBuilder::new(&mut self.ctx.func, &mut self.func_ctx);

            // Create entry block
            let entry_block = builder.create_block();
            builder.append_block_params_for_function_params(entry_block);
            builder.switch_to_block(entry_block);
            builder.seal_block(entry_block);

            // Get parameters
            let _ctx_ptr = builder.block_params(entry_block)[0];
            let _ctx_len = builder.block_params(entry_block)[1];
            let result_ptr = builder.block_params(entry_block)[2];

            // Compile expression to IR
            match compile_expr_to_ir(&mut builder, expr, result_ptr) {
                Ok(()) => {
                    // Return success
                    let zero = builder.ins().iconst(types::I32, 0);
                    builder.ins().return_(&[zero]);
                }
                Err(_) => {
                    // Return error
                    let error = builder
                        .ins()
                        .iconst(types::I32, JITErrorCode::InvalidOperation as i64);
                    builder.ins().return_(&[error]);
                }
            }

            builder.finalize();
        }

        // Compile to machine code
        self.module
            .define_function(func_id, &mut self.ctx)
            .map_err(|e| OrdoError::eval_error(format!("Failed to define function: {}", e)))?;

        self.module.clear_context(&mut self.ctx);
        self.module
            .finalize_definitions()
            .map_err(|e| OrdoError::eval_error(format!("Failed to finalize definitions: {}", e)))?;

        // Get function pointer
        let func_ptr = self.module.get_finalized_function(func_id);
        let code_size = 0; // TODO: Get actual code size

        let compiled = CompiledFunction {
            func_ptr,
            expr_hash: hash,
            code_size,
        };

        self.functions.insert(hash, compiled);

        let duration = start.elapsed();
        self.stats.successful_compiles += 1;
        self.stats.total_compile_time_ns += duration.as_nanos() as u64;
        self.stats.total_code_size += code_size;

        Ok(self.functions.get(&hash).unwrap())
    }

    /// Check if an expression can be JIT-compiled
    pub fn is_jit_compilable(expr: &Expr) -> bool {
        match expr {
            // Literals are always compilable
            Expr::Literal(_) => true,

            // Field access requires runtime context lookup
            Expr::Field(_) => true,

            // Binary operations are compilable if both operands are
            Expr::Binary { left, right, .. } => {
                Self::is_jit_compilable(left) && Self::is_jit_compilable(right)
            }

            // Unary operations are compilable if operand is
            Expr::Unary { operand, .. } => Self::is_jit_compilable(operand),

            // Conditional expressions
            Expr::Conditional {
                condition,
                then_branch,
                else_branch,
            } => {
                Self::is_jit_compilable(condition)
                    && Self::is_jit_compilable(then_branch)
                    && Self::is_jit_compilable(else_branch)
            }

            // Function calls - only built-in numeric functions for now
            Expr::Call { name, args } => {
                let supported_funcs = ["abs", "min", "max", "floor", "ceil", "round"];
                supported_funcs.contains(&name.as_str()) && args.iter().all(Self::is_jit_compilable)
            }

            // Arrays, objects, exists, and coalesce are not yet supported
            Expr::Array(_) | Expr::Object(_) | Expr::Exists(_) | Expr::Coalesce(_) => false,
        }
    }
}

// ==================== Standalone Compilation Functions ====================
// These are standalone functions to avoid borrow conflicts with FunctionBuilder

/// Compile an expression to Cranelift IR
fn compile_expr_to_ir(
    builder: &mut FunctionBuilder,
    expr: &Expr,
    result_ptr: cranelift::prelude::Value,
) -> Result<()> {
    match expr {
        Expr::Literal(value) => {
            let val = compile_literal(builder, value)?;
            // Store result as f64
            builder.ins().store(MemFlags::new(), val, result_ptr, 0);
            Ok(())
        }

        Expr::Binary { left, op, right } => {
            // For binary ops, we need to recursively compile and use temp storage
            let left_val = compile_expr_value(builder, left)?;
            let right_val = compile_expr_value(builder, right)?;

            let result = compile_binary_op(builder, *op, left_val, right_val)?;
            builder.ins().store(MemFlags::new(), result, result_ptr, 0);
            Ok(())
        }

        Expr::Unary { op, operand } => {
            let val = compile_expr_value(builder, operand)?;
            let result = compile_unary_op(builder, *op, val)?;
            builder.ins().store(MemFlags::new(), result, result_ptr, 0);
            Ok(())
        }

        Expr::Field(_) => {
            // Field access requires runtime context lookup
            // For now, return 0 as placeholder
            let zero = builder.ins().f64const(0.0);
            builder.ins().store(MemFlags::new(), zero, result_ptr, 0);
            Ok(())
        }

        Expr::Conditional {
            condition,
            then_branch,
            else_branch,
        } => compile_conditional(builder, condition, then_branch, else_branch, result_ptr),

        Expr::Call { name, args } => compile_function_call(builder, name, args, result_ptr),

        _ => Err(OrdoError::eval_error(
            "Unsupported expression for JIT".to_string(),
        )),
    }
}

/// Compile an expression and return the value
fn compile_expr_value(
    builder: &mut FunctionBuilder,
    expr: &Expr,
) -> Result<cranelift::prelude::Value> {
    match expr {
        Expr::Literal(value) => compile_literal(builder, value),

        Expr::Binary { left, op, right } => {
            let left_val = compile_expr_value(builder, left)?;
            let right_val = compile_expr_value(builder, right)?;
            compile_binary_op(builder, *op, left_val, right_val)
        }

        Expr::Unary { op, operand } => {
            let val = compile_expr_value(builder, operand)?;
            compile_unary_op(builder, *op, val)
        }

        Expr::Field(_) => {
            // Field access - return 0 for now
            Ok(builder.ins().f64const(0.0))
        }

        Expr::Call { name, args } => compile_function_call_value(builder, name, args),

        _ => Err(OrdoError::eval_error(
            "Unsupported expression for JIT value".to_string(),
        )),
    }
}

/// Compile a literal value
fn compile_literal(
    builder: &mut FunctionBuilder,
    value: &Value,
) -> Result<cranelift::prelude::Value> {
    match value {
        Value::Null => Ok(builder.ins().f64const(0.0)),
        Value::Bool(b) => Ok(builder.ins().f64const(if *b { 1.0 } else { 0.0 })),
        Value::Int(n) => Ok(builder.ins().f64const(*n as f64)),
        Value::Float(f) => Ok(builder.ins().f64const(*f)),
        Value::String(_) => {
            // Strings not supported in numeric JIT
            Err(OrdoError::eval_error(
                "String literals not supported in JIT".to_string(),
            ))
        }
        Value::Array(_) | Value::Object(_) => Err(OrdoError::eval_error(
            "Complex types not supported in JIT".to_string(),
        )),
    }
}

/// Compile a binary operation
fn compile_binary_op(
    builder: &mut FunctionBuilder,
    op: BinaryOp,
    left: cranelift::prelude::Value,
    right: cranelift::prelude::Value,
) -> Result<cranelift::prelude::Value> {
    let result = match op {
        BinaryOp::Add => builder.ins().fadd(left, right),
        BinaryOp::Sub => builder.ins().fsub(left, right),
        BinaryOp::Mul => builder.ins().fmul(left, right),
        BinaryOp::Div => builder.ins().fdiv(left, right),
        BinaryOp::Mod => {
            // f64 modulo: a - floor(a/b) * b
            let div = builder.ins().fdiv(left, right);
            let floor = builder.ins().floor(div);
            let prod = builder.ins().fmul(floor, right);
            builder.ins().fsub(left, prod)
        }

        // Comparison operators return 1.0 or 0.0
        BinaryOp::Eq => {
            let cmp = builder.ins().fcmp(FloatCC::Equal, left, right);
            let one = builder.ins().f64const(1.0);
            let zero = builder.ins().f64const(0.0);
            builder.ins().select(cmp, one, zero)
        }
        BinaryOp::Ne => {
            let cmp = builder.ins().fcmp(FloatCC::NotEqual, left, right);
            let one = builder.ins().f64const(1.0);
            let zero = builder.ins().f64const(0.0);
            builder.ins().select(cmp, one, zero)
        }
        BinaryOp::Lt => {
            let cmp = builder.ins().fcmp(FloatCC::LessThan, left, right);
            let one = builder.ins().f64const(1.0);
            let zero = builder.ins().f64const(0.0);
            builder.ins().select(cmp, one, zero)
        }
        BinaryOp::Le => {
            let cmp = builder.ins().fcmp(FloatCC::LessThanOrEqual, left, right);
            let one = builder.ins().f64const(1.0);
            let zero = builder.ins().f64const(0.0);
            builder.ins().select(cmp, one, zero)
        }
        BinaryOp::Gt => {
            let cmp = builder.ins().fcmp(FloatCC::GreaterThan, left, right);
            let one = builder.ins().f64const(1.0);
            let zero = builder.ins().f64const(0.0);
            builder.ins().select(cmp, one, zero)
        }
        BinaryOp::Ge => {
            let cmp = builder.ins().fcmp(FloatCC::GreaterThanOrEqual, left, right);
            let one = builder.ins().f64const(1.0);
            let zero = builder.ins().f64const(0.0);
            builder.ins().select(cmp, one, zero)
        }

        // Logical operators (treat non-zero as true)
        BinaryOp::And => {
            let zero = builder.ins().f64const(0.0);
            let left_bool = builder.ins().fcmp(FloatCC::NotEqual, left, zero);
            let right_bool = builder.ins().fcmp(FloatCC::NotEqual, right, zero);
            let and_result = builder.ins().band(left_bool, right_bool);
            let one = builder.ins().f64const(1.0);
            builder.ins().select(and_result, one, zero)
        }
        BinaryOp::Or => {
            let zero = builder.ins().f64const(0.0);
            let left_bool = builder.ins().fcmp(FloatCC::NotEqual, left, zero);
            let right_bool = builder.ins().fcmp(FloatCC::NotEqual, right, zero);
            let or_result = builder.ins().bor(left_bool, right_bool);
            let one = builder.ins().f64const(1.0);
            builder.ins().select(or_result, one, zero)
        }

        // String operations not supported
        BinaryOp::In | BinaryOp::NotIn | BinaryOp::Contains => {
            return Err(OrdoError::eval_error(
                "String operations not supported in JIT".to_string(),
            ))
        }
    };

    Ok(result)
}

/// Compile a unary operation
fn compile_unary_op(
    builder: &mut FunctionBuilder,
    op: UnaryOp,
    operand: cranelift::prelude::Value,
) -> Result<cranelift::prelude::Value> {
    let result = match op {
        UnaryOp::Not => {
            let zero = builder.ins().f64const(0.0);
            let is_zero = builder.ins().fcmp(FloatCC::Equal, operand, zero);
            let one = builder.ins().f64const(1.0);
            builder.ins().select(is_zero, one, zero)
        }
        UnaryOp::Neg => builder.ins().fneg(operand),
    };

    Ok(result)
}

/// Compile a conditional expression
fn compile_conditional(
    builder: &mut FunctionBuilder,
    condition: &Expr,
    then_expr: &Expr,
    else_expr: &Expr,
    result_ptr: cranelift::prelude::Value,
) -> Result<()> {
    let cond_val = compile_expr_value(builder, condition)?;

    // Create blocks for then, else, and merge
    let then_block = builder.create_block();
    let else_block = builder.create_block();
    let merge_block = builder.create_block();

    // Branch based on condition
    let zero = builder.ins().f64const(0.0);
    let cond_bool = builder.ins().fcmp(FloatCC::NotEqual, cond_val, zero);
    builder
        .ins()
        .brif(cond_bool, then_block, &[], else_block, &[]);

    // Then block
    builder.switch_to_block(then_block);
    builder.seal_block(then_block);
    let then_val = compile_expr_value(builder, then_expr)?;
    builder
        .ins()
        .store(MemFlags::new(), then_val, result_ptr, 0);
    builder.ins().jump(merge_block, &[]);

    // Else block
    builder.switch_to_block(else_block);
    builder.seal_block(else_block);
    let else_val = compile_expr_value(builder, else_expr)?;
    builder
        .ins()
        .store(MemFlags::new(), else_val, result_ptr, 0);
    builder.ins().jump(merge_block, &[]);

    // Merge block
    builder.switch_to_block(merge_block);
    builder.seal_block(merge_block);

    Ok(())
}

/// Compile a function call
fn compile_function_call(
    builder: &mut FunctionBuilder,
    name: &str,
    args: &[Expr],
    result_ptr: cranelift::prelude::Value,
) -> Result<()> {
    let result = compile_function_call_value(builder, name, args)?;
    builder.ins().store(MemFlags::new(), result, result_ptr, 0);
    Ok(())
}

/// Compile a function call and return the value
fn compile_function_call_value(
    builder: &mut FunctionBuilder,
    name: &str,
    args: &[Expr],
) -> Result<cranelift::prelude::Value> {
    match name {
        "abs" => {
            if args.len() != 1 {
                return Err(OrdoError::eval_error("abs requires 1 argument".to_string()));
            }
            let val = compile_expr_value(builder, &args[0])?;
            Ok(builder.ins().fabs(val))
        }

        "floor" => {
            if args.len() != 1 {
                return Err(OrdoError::eval_error(
                    "floor requires 1 argument".to_string(),
                ));
            }
            let val = compile_expr_value(builder, &args[0])?;
            Ok(builder.ins().floor(val))
        }

        "ceil" => {
            if args.len() != 1 {
                return Err(OrdoError::eval_error(
                    "ceil requires 1 argument".to_string(),
                ));
            }
            let val = compile_expr_value(builder, &args[0])?;
            Ok(builder.ins().ceil(val))
        }

        "round" => {
            if args.len() != 1 {
                return Err(OrdoError::eval_error(
                    "round requires 1 argument".to_string(),
                ));
            }
            let val = compile_expr_value(builder, &args[0])?;
            Ok(builder.ins().nearest(val))
        }

        "min" => {
            if args.len() != 2 {
                return Err(OrdoError::eval_error(
                    "min requires 2 arguments".to_string(),
                ));
            }
            let a = compile_expr_value(builder, &args[0])?;
            let b = compile_expr_value(builder, &args[1])?;
            Ok(builder.ins().fmin(a, b))
        }

        "max" => {
            if args.len() != 2 {
                return Err(OrdoError::eval_error(
                    "max requires 2 arguments".to_string(),
                ));
            }
            let a = compile_expr_value(builder, &args[0])?;
            let b = compile_expr_value(builder, &args[1])?;
            Ok(builder.ins().fmax(a, b))
        }

        _ => Err(OrdoError::eval_error(format!(
            "Function '{}' not supported in JIT",
            name
        ))),
    }
}

impl Default for JITCompiler {
    fn default() -> Self {
        Self::new().expect("Failed to create default JIT compiler")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_jit_compiler_creation() {
        let compiler = JITCompiler::new();
        assert!(compiler.is_ok());
    }

    #[test]
    fn test_is_jit_compilable() {
        // Literal - compilable
        let lit = Expr::Literal(Value::Float(42.0));
        assert!(JITCompiler::is_jit_compilable(&lit));

        // Binary arithmetic - compilable
        let add = Expr::Binary {
            left: Box::new(Expr::Literal(Value::Float(1.0))),
            op: BinaryOp::Add,
            right: Box::new(Expr::Literal(Value::Float(2.0))),
        };
        assert!(JITCompiler::is_jit_compilable(&add));

        // Array - not compilable
        let arr = Expr::Array(vec![]);
        assert!(!JITCompiler::is_jit_compilable(&arr));
    }

    #[test]
    fn test_compile_simple_expression() {
        let mut compiler = JITCompiler::new().unwrap();

        // 1 + 2
        let expr = Expr::Binary {
            left: Box::new(Expr::Literal(Value::Float(1.0))),
            op: BinaryOp::Add,
            right: Box::new(Expr::Literal(Value::Float(2.0))),
        };

        let result = compiler.compile(&expr, 12345);
        assert!(result.is_ok());

        let compiled = result.unwrap();
        assert_eq!(compiled.expr_hash, 12345);
    }

    #[test]
    fn test_compile_complex_expression() {
        let mut compiler = JITCompiler::new().unwrap();

        // (1 + 2) * 3 - abs(-4)
        let expr = Expr::Binary {
            left: Box::new(Expr::Binary {
                left: Box::new(Expr::Binary {
                    left: Box::new(Expr::Literal(Value::Float(1.0))),
                    op: BinaryOp::Add,
                    right: Box::new(Expr::Literal(Value::Float(2.0))),
                }),
                op: BinaryOp::Mul,
                right: Box::new(Expr::Literal(Value::Float(3.0))),
            }),
            op: BinaryOp::Sub,
            right: Box::new(Expr::Call {
                name: "abs".to_string(),
                args: vec![Expr::Unary {
                    op: UnaryOp::Neg,
                    operand: Box::new(Expr::Literal(Value::Float(4.0))),
                }],
            }),
        };

        let result = compiler.compile(&expr, 67890);
        assert!(result.is_ok());
    }

    #[test]
    fn test_compile_comparison() {
        let mut compiler = JITCompiler::new().unwrap();

        // 5 > 3
        let expr = Expr::Binary {
            left: Box::new(Expr::Literal(Value::Float(5.0))),
            op: BinaryOp::Gt,
            right: Box::new(Expr::Literal(Value::Float(3.0))),
        };

        let result = compiler.compile(&expr, 11111);
        assert!(result.is_ok());
    }

    #[test]
    fn test_jit_stats() {
        let mut compiler = JITCompiler::new().unwrap();

        let expr = Expr::Literal(Value::Float(42.0));
        compiler.compile(&expr, 1).unwrap();

        let stats = compiler.stats();
        assert_eq!(stats.successful_compiles, 1);
        assert_eq!(stats.failed_compiles, 0);
    }
}
