//! Schema-Aware JIT Compiler for Expressions
//!
//! Compiles Expr AST to native machine code with direct field access
//! using compile-time known field offsets from the schema.
//!
//! # Architecture
//!
//! The Schema-Aware JIT compiler generates functions with the signature:
//! `fn(ctx_ptr: *const u8, result_ptr: *mut u8) -> i32`
//!
//! Where:
//! - `ctx_ptr`: Pointer to a TypedContext struct (NOT serialized data)
//! - `result_ptr`: Pointer to write the result value (f64)
//! - Returns: 0 on success, error code on failure
//!
//! # No Trampolines
//!
//! Unlike the legacy compiler, this version does NOT use trampolines.
//! Field access is compiled directly as memory loads with compile-time offsets:
//!
//! ```text
//! JIT Code: ldr d0, [ctx_ptr, #offset]  // Direct memory access!
//! ```
//!
//! This provides ~15-30x faster field access compared to trampoline-based approach.

use crate::context::{FieldType, MessageSchema, ResolvedField, Value};
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
pub enum SchemaJITErrorCode {
    Success = 0,
    NullPointer = 1,
    TypeMismatch = 2,
    DivisionByZero = 3,
    FieldNotFound = 4,
    InvalidOperation = 5,
    Overflow = 6,
}

/// Result of a schema-aware JIT compilation
#[derive(Clone)]
pub struct SchemaCompiledFunction {
    /// Function pointer
    pub func_ptr: *const u8,
    /// Source expression hash
    pub expr_hash: u64,
    /// Schema used for compilation
    pub schema_name: String,
    /// Estimated code size in bytes
    pub code_size: usize,
    /// Fields accessed by this function
    pub accessed_fields: Vec<String>,
}

// Safety: The function pointer is valid for the lifetime of JITModule
unsafe impl Send for SchemaCompiledFunction {}
unsafe impl Sync for SchemaCompiledFunction {}

impl SchemaCompiledFunction {
    /// Execute the compiled function with a typed context
    ///
    /// # Safety
    /// The caller must ensure:
    /// - The context pointer points to a valid struct matching the schema
    /// - The struct has the same layout as when the function was compiled
    #[inline]
    pub unsafe fn call_typed<T>(&self, ctx: &T) -> Result<f64> {
        let mut result_buf = [0u8; 8];

        let func: extern "C" fn(*const u8, *mut u8) -> i32 = std::mem::transmute(self.func_ptr);

        let error_code = func(ctx as *const T as *const u8, result_buf.as_mut_ptr());

        if error_code != 0 {
            return Err(OrdoError::eval_error(format!(
                "Schema JIT execution failed with code: {}",
                error_code
            )));
        }

        Ok(f64::from_le_bytes(result_buf))
    }

    /// Execute and return as Value
    ///
    /// # Safety
    ///
    /// The caller must ensure that `ctx` is a valid typed context matching
    /// the schema this function was compiled for.
    #[inline]
    pub unsafe fn call_typed_value<T>(&self, ctx: &T) -> Result<Value> {
        self.call_typed(ctx).map(Value::Float)
    }
}

/// Schema-Aware JIT Compiler using Cranelift
pub struct SchemaJITCompiler {
    /// Cranelift JIT module
    module: JITModule,
    /// Codegen context (reusable)
    ctx: codegen::Context,
    /// Function builder context (reusable)
    func_ctx: FunctionBuilderContext,
    /// Compiled functions by (expr_hash, schema_name)
    functions: HashMap<(u64, String), SchemaCompiledFunction>,
    /// Statistics
    stats: SchemaJITStats,
}

/// JIT compilation statistics
#[derive(Debug, Default, Clone)]
pub struct SchemaJITStats {
    /// Number of successful compilations
    pub successful_compiles: u64,
    /// Number of failed compilations
    pub failed_compiles: u64,
    /// Total compilation time in nanoseconds
    pub total_compile_time_ns: u64,
    /// Total code size generated
    pub total_code_size: usize,
    /// Number of schema-aware compilations
    pub schema_compiles: u64,
}

impl SchemaJITCompiler {
    /// Create a new Schema-Aware JIT compiler
    pub fn new() -> Result<Self> {
        let mut flag_builder = settings::builder();
        flag_builder.set("use_colocated_libcalls", "false").unwrap();
        flag_builder.set("is_pic", "false").unwrap();

        let isa_builder = cranelift_native::builder()
            .map_err(|e| OrdoError::eval_error(format!("Failed to create ISA builder: {}", e)))?;

        let isa = isa_builder
            .finish(settings::Flags::new(flag_builder))
            .map_err(|e| OrdoError::eval_error(format!("Failed to create ISA: {}", e)))?;

        let builder = JITBuilder::with_isa(isa, cranelift_module::default_libcall_names());
        let module = JITModule::new(builder);
        let ctx = module.make_context();
        let func_ctx = FunctionBuilderContext::new();

        Ok(Self {
            module,
            ctx,
            func_ctx,
            functions: HashMap::new(),
            stats: SchemaJITStats::default(),
        })
    }

    /// Get compilation statistics
    pub fn stats(&self) -> &SchemaJITStats {
        &self.stats
    }

    /// Check if an expression has been compiled for a schema
    pub fn is_compiled(&self, hash: u64, schema_name: &str) -> bool {
        self.functions
            .contains_key(&(hash, schema_name.to_string()))
    }

    /// Get a compiled function
    pub fn get(&self, hash: u64, schema_name: &str) -> Option<&SchemaCompiledFunction> {
        self.functions.get(&(hash, schema_name.to_string()))
    }

    /// Check if an expression can be compiled with a schema
    pub fn can_compile_with_schema(expr: &Expr, schema: &MessageSchema) -> bool {
        // Collect all field accesses
        let mut fields = Vec::new();
        Self::collect_field_accesses(expr, &mut fields);

        // All fields must exist in schema and be numeric
        for field in &fields {
            if let Some(resolved) = schema.resolve_field_path(field) {
                if !resolved.field_type.is_jit_numeric() {
                    return false;
                }
            } else {
                return false;
            }
        }

        // Check that expression structure is supported
        Self::is_supported_expr(expr)
    }

    /// Collect all field accesses from an expression
    fn collect_field_accesses(expr: &Expr, fields: &mut Vec<String>) {
        match expr {
            Expr::Field(name) => {
                if !fields.contains(name) {
                    fields.push(name.clone());
                }
            }
            Expr::Binary { left, right, .. } => {
                Self::collect_field_accesses(left, fields);
                Self::collect_field_accesses(right, fields);
            }
            Expr::Unary { operand, .. } => {
                Self::collect_field_accesses(operand, fields);
            }
            Expr::Conditional {
                condition,
                then_branch,
                else_branch,
            } => {
                Self::collect_field_accesses(condition, fields);
                Self::collect_field_accesses(then_branch, fields);
                Self::collect_field_accesses(else_branch, fields);
            }
            Expr::Call { args, .. } => {
                for arg in args {
                    Self::collect_field_accesses(arg, fields);
                }
            }
            Expr::Exists(field_name) => {
                if !fields.contains(field_name) {
                    fields.push(field_name.clone());
                }
            }
            _ => {}
        }
    }

    /// Check if expression structure is supported
    fn is_supported_expr(expr: &Expr) -> bool {
        match expr {
            Expr::Literal(v) => matches!(
                v,
                Value::Null | Value::Bool(_) | Value::Int(_) | Value::Float(_)
            ),
            Expr::Field(_) => true,
            Expr::Binary { left, right, op } => {
                // String operations not supported
                !matches!(op, BinaryOp::In | BinaryOp::NotIn | BinaryOp::Contains)
                    && Self::is_supported_expr(left)
                    && Self::is_supported_expr(right)
            }
            Expr::Unary { operand, .. } => Self::is_supported_expr(operand),
            Expr::Conditional {
                condition,
                then_branch,
                else_branch,
            } => {
                Self::is_supported_expr(condition)
                    && Self::is_supported_expr(then_branch)
                    && Self::is_supported_expr(else_branch)
            }
            Expr::Call { name, args } => {
                // Only math functions are supported (no field-based array ops)
                let supported = ["abs", "min", "max", "floor", "ceil", "round", "sqrt", "pow"];
                supported.contains(&name.as_str()) && args.iter().all(Self::is_supported_expr)
            }
            // Not supported: Array, Object, Coalesce, Exists
            _ => false,
        }
    }

    /// Compile an expression with a schema
    pub fn compile_with_schema(
        &mut self,
        expr: &Expr,
        hash: u64,
        schema: &MessageSchema,
    ) -> Result<&SchemaCompiledFunction> {
        let key = (hash, schema.name.clone());
        if self.functions.contains_key(&key) {
            return Ok(self.functions.get(&key).unwrap());
        }

        let start = std::time::Instant::now();

        // Validate expression is compilable with this schema
        if !Self::can_compile_with_schema(expr, schema) {
            self.stats.failed_compiles += 1;
            return Err(OrdoError::eval_error(
                "Expression is not compilable with the given schema".to_string(),
            ));
        }

        // Collect field accesses and resolve offsets
        let mut field_names = Vec::new();
        Self::collect_field_accesses(expr, &mut field_names);

        let field_offsets: HashMap<String, ResolvedField> = field_names
            .iter()
            .filter_map(|name| {
                schema
                    .resolve_field_path(name)
                    .map(|resolved| (name.clone(), resolved))
            })
            .collect();

        // Create function signature: fn(ctx_ptr: ptr, result_ptr: ptr) -> i32
        let ptr_type = self.module.target_config().pointer_type();
        let mut sig = self.module.make_signature();
        sig.params.push(AbiParam::new(ptr_type)); // ctx_ptr
        sig.params.push(AbiParam::new(ptr_type)); // result_ptr
        sig.returns.push(AbiParam::new(types::I32)); // error code

        // Declare function
        let func_name = format!("schema_jit_{}_{}", schema.name, hash);
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
            let ctx_ptr = builder.block_params(entry_block)[0];
            let result_ptr = builder.block_params(entry_block)[1];

            // Create compilation context
            let compile_ctx = SchemaCompileContext {
                field_offsets: &field_offsets,
                ptr_type,
            };

            // Compile expression to IR
            match compile_expr_to_ir(&mut builder, expr, ctx_ptr, result_ptr, &compile_ctx) {
                Ok(()) => {
                    // Return success
                    let zero = builder.ins().iconst(types::I32, 0);
                    builder.ins().return_(&[zero]);
                }
                Err(_) => {
                    // Return error
                    let error = builder
                        .ins()
                        .iconst(types::I32, SchemaJITErrorCode::InvalidOperation as i64);
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

        let compiled = SchemaCompiledFunction {
            func_ptr,
            expr_hash: hash,
            schema_name: schema.name.clone(),
            code_size: 0, // TODO: Get actual code size
            accessed_fields: field_names,
        };

        self.functions.insert(key.clone(), compiled);

        let duration = start.elapsed();
        self.stats.successful_compiles += 1;
        self.stats.schema_compiles += 1;
        self.stats.total_compile_time_ns += duration.as_nanos() as u64;

        Ok(self.functions.get(&key).unwrap())
    }
}

impl Default for SchemaJITCompiler {
    fn default() -> Self {
        Self::new().expect("Failed to create default Schema JIT compiler")
    }
}

// ==================== Compilation Context ====================

/// Compilation context with schema information
struct SchemaCompileContext<'a> {
    field_offsets: &'a HashMap<String, ResolvedField>,
    #[allow(dead_code)]
    ptr_type: Type,
}

// ==================== Expression Compilation ====================

/// Compile an expression to Cranelift IR
fn compile_expr_to_ir(
    builder: &mut FunctionBuilder,
    expr: &Expr,
    ctx_ptr: cranelift::prelude::Value,
    result_ptr: cranelift::prelude::Value,
    compile_ctx: &SchemaCompileContext,
) -> Result<()> {
    match expr {
        Expr::Literal(value) => {
            let val = compile_literal(builder, value)?;
            builder.ins().store(MemFlags::new(), val, result_ptr, 0);
            Ok(())
        }

        Expr::Binary { left, op, right } => {
            let left_val = compile_expr_value(builder, left, ctx_ptr, compile_ctx)?;
            let right_val = compile_expr_value(builder, right, ctx_ptr, compile_ctx)?;
            let result = compile_binary_op(builder, *op, left_val, right_val)?;
            builder.ins().store(MemFlags::new(), result, result_ptr, 0);
            Ok(())
        }

        Expr::Unary { op, operand } => {
            let val = compile_expr_value(builder, operand, ctx_ptr, compile_ctx)?;
            let result = compile_unary_op(builder, *op, val)?;
            builder.ins().store(MemFlags::new(), result, result_ptr, 0);
            Ok(())
        }

        Expr::Field(name) => {
            let val = compile_field_access_direct(builder, ctx_ptr, name, compile_ctx)?;
            builder.ins().store(MemFlags::new(), val, result_ptr, 0);
            Ok(())
        }

        Expr::Conditional {
            condition,
            then_branch,
            else_branch,
        } => compile_conditional(
            builder,
            condition,
            then_branch,
            else_branch,
            ctx_ptr,
            result_ptr,
            compile_ctx,
        ),

        Expr::Call { name, args } => {
            compile_math_function(builder, name, args, ctx_ptr, result_ptr, compile_ctx)
        }

        _ => Err(OrdoError::eval_error(
            "Unsupported expression for Schema JIT".to_string(),
        )),
    }
}

/// Compile an expression and return the value
fn compile_expr_value(
    builder: &mut FunctionBuilder,
    expr: &Expr,
    ctx_ptr: cranelift::prelude::Value,
    compile_ctx: &SchemaCompileContext,
) -> Result<cranelift::prelude::Value> {
    match expr {
        Expr::Literal(value) => compile_literal(builder, value),

        Expr::Binary { left, op, right } => {
            let left_val = compile_expr_value(builder, left, ctx_ptr, compile_ctx)?;
            let right_val = compile_expr_value(builder, right, ctx_ptr, compile_ctx)?;
            compile_binary_op(builder, *op, left_val, right_val)
        }

        Expr::Unary { op, operand } => {
            let val = compile_expr_value(builder, operand, ctx_ptr, compile_ctx)?;
            compile_unary_op(builder, *op, val)
        }

        Expr::Field(name) => compile_field_access_direct(builder, ctx_ptr, name, compile_ctx),

        Expr::Call { name, args } => {
            compile_math_function_value(builder, name, args, ctx_ptr, compile_ctx)
        }

        _ => Err(OrdoError::eval_error(
            "Unsupported expression for Schema JIT value".to_string(),
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
        _ => Err(OrdoError::eval_error(
            "Unsupported literal type for Schema JIT".to_string(),
        )),
    }
}

/// Compile direct field access (the key optimization!)
fn compile_field_access_direct(
    builder: &mut FunctionBuilder,
    ctx_ptr: cranelift::prelude::Value,
    field_name: &str,
    compile_ctx: &SchemaCompileContext,
) -> Result<cranelift::prelude::Value> {
    let resolved = compile_ctx.field_offsets.get(field_name).ok_or_else(|| {
        OrdoError::eval_error(format!("Field '{}' not found in schema", field_name))
    })?;

    let offset = resolved.offset;

    // Generate direct memory access based on field type
    match &resolved.field_type {
        FieldType::Float64 => {
            // Direct f64 load: ldr d0, [ctx_ptr, #offset]
            Ok(builder
                .ins()
                .load(types::F64, MemFlags::new(), ctx_ptr, offset as i32))
        }
        FieldType::Float32 => {
            // Load f32 and promote to f64
            let f32_val = builder
                .ins()
                .load(types::F32, MemFlags::new(), ctx_ptr, offset as i32);
            Ok(builder.ins().fpromote(types::F64, f32_val))
        }
        FieldType::Int64 => {
            // Load i64 and convert to f64
            let int_val = builder
                .ins()
                .load(types::I64, MemFlags::new(), ctx_ptr, offset as i32);
            Ok(builder.ins().fcvt_from_sint(types::F64, int_val))
        }
        FieldType::Int32 => {
            // Load i32, sign-extend to i64, then convert to f64
            let int_val = builder
                .ins()
                .load(types::I32, MemFlags::new(), ctx_ptr, offset as i32);
            let int64 = builder.ins().sextend(types::I64, int_val);
            Ok(builder.ins().fcvt_from_sint(types::F64, int64))
        }
        FieldType::UInt64 => {
            let int_val = builder
                .ins()
                .load(types::I64, MemFlags::new(), ctx_ptr, offset as i32);
            Ok(builder.ins().fcvt_from_uint(types::F64, int_val))
        }
        FieldType::UInt32 => {
            let int_val = builder
                .ins()
                .load(types::I32, MemFlags::new(), ctx_ptr, offset as i32);
            let int64 = builder.ins().uextend(types::I64, int_val);
            Ok(builder.ins().fcvt_from_uint(types::F64, int64))
        }
        FieldType::Bool => {
            // Load bool (1 byte), extend to i64, convert to f64
            let bool_val = builder
                .ins()
                .load(types::I8, MemFlags::new(), ctx_ptr, offset as i32);
            let int64 = builder.ins().uextend(types::I64, bool_val);
            Ok(builder.ins().fcvt_from_uint(types::F64, int64))
        }
        FieldType::Enum(_) => {
            // Enums are stored as i32
            let int_val = builder
                .ins()
                .load(types::I32, MemFlags::new(), ctx_ptr, offset as i32);
            let int64 = builder.ins().sextend(types::I64, int_val);
            Ok(builder.ins().fcvt_from_sint(types::F64, int64))
        }
        _ => Err(OrdoError::eval_error(format!(
            "Field type {:?} not supported for Schema JIT",
            resolved.field_type
        ))),
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

        // Logical operators
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
                "String operations not supported in Schema JIT".to_string(),
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
    ctx_ptr: cranelift::prelude::Value,
    result_ptr: cranelift::prelude::Value,
    compile_ctx: &SchemaCompileContext,
) -> Result<()> {
    let cond_val = compile_expr_value(builder, condition, ctx_ptr, compile_ctx)?;

    // Create blocks
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
    let then_val = compile_expr_value(builder, then_expr, ctx_ptr, compile_ctx)?;
    builder
        .ins()
        .store(MemFlags::new(), then_val, result_ptr, 0);
    builder.ins().jump(merge_block, &[]);

    // Else block
    builder.switch_to_block(else_block);
    builder.seal_block(else_block);
    let else_val = compile_expr_value(builder, else_expr, ctx_ptr, compile_ctx)?;
    builder
        .ins()
        .store(MemFlags::new(), else_val, result_ptr, 0);
    builder.ins().jump(merge_block, &[]);

    // Merge block
    builder.switch_to_block(merge_block);
    builder.seal_block(merge_block);

    Ok(())
}

/// Compile a math function call
fn compile_math_function(
    builder: &mut FunctionBuilder,
    name: &str,
    args: &[Expr],
    ctx_ptr: cranelift::prelude::Value,
    result_ptr: cranelift::prelude::Value,
    compile_ctx: &SchemaCompileContext,
) -> Result<()> {
    let result = compile_math_function_value(builder, name, args, ctx_ptr, compile_ctx)?;
    builder.ins().store(MemFlags::new(), result, result_ptr, 0);
    Ok(())
}

/// Compile a math function and return the value
fn compile_math_function_value(
    builder: &mut FunctionBuilder,
    name: &str,
    args: &[Expr],
    ctx_ptr: cranelift::prelude::Value,
    compile_ctx: &SchemaCompileContext,
) -> Result<cranelift::prelude::Value> {
    match name {
        "abs" => {
            if args.len() != 1 {
                return Err(OrdoError::eval_error("abs requires 1 argument".to_string()));
            }
            let val = compile_expr_value(builder, &args[0], ctx_ptr, compile_ctx)?;
            Ok(builder.ins().fabs(val))
        }

        "floor" => {
            if args.len() != 1 {
                return Err(OrdoError::eval_error(
                    "floor requires 1 argument".to_string(),
                ));
            }
            let val = compile_expr_value(builder, &args[0], ctx_ptr, compile_ctx)?;
            Ok(builder.ins().floor(val))
        }

        "ceil" => {
            if args.len() != 1 {
                return Err(OrdoError::eval_error(
                    "ceil requires 1 argument".to_string(),
                ));
            }
            let val = compile_expr_value(builder, &args[0], ctx_ptr, compile_ctx)?;
            Ok(builder.ins().ceil(val))
        }

        "round" => {
            if args.len() != 1 {
                return Err(OrdoError::eval_error(
                    "round requires 1 argument".to_string(),
                ));
            }
            let val = compile_expr_value(builder, &args[0], ctx_ptr, compile_ctx)?;
            Ok(builder.ins().nearest(val))
        }

        "sqrt" => {
            if args.len() != 1 {
                return Err(OrdoError::eval_error(
                    "sqrt requires 1 argument".to_string(),
                ));
            }
            let val = compile_expr_value(builder, &args[0], ctx_ptr, compile_ctx)?;
            Ok(builder.ins().sqrt(val))
        }

        "min" => {
            if args.len() != 2 {
                return Err(OrdoError::eval_error(
                    "min requires 2 arguments".to_string(),
                ));
            }
            let a = compile_expr_value(builder, &args[0], ctx_ptr, compile_ctx)?;
            let b = compile_expr_value(builder, &args[1], ctx_ptr, compile_ctx)?;
            Ok(builder.ins().fmin(a, b))
        }

        "max" => {
            if args.len() != 2 {
                return Err(OrdoError::eval_error(
                    "max requires 2 arguments".to_string(),
                ));
            }
            let a = compile_expr_value(builder, &args[0], ctx_ptr, compile_ctx)?;
            let b = compile_expr_value(builder, &args[1], ctx_ptr, compile_ctx)?;
            Ok(builder.ins().fmax(a, b))
        }

        // For pow, we need to use a runtime call since Cranelift doesn't have a native pow
        // For now, we approximate using exp and log: pow(a, b) = exp(b * log(a))
        "pow" => {
            if args.len() != 2 {
                return Err(OrdoError::eval_error(
                    "pow requires 2 arguments".to_string(),
                ));
            }
            // Note: pow is complex to implement in pure IR
            // For a complete implementation, we would need to call a runtime function
            // For now, return an error suggesting to use other approaches
            Err(OrdoError::eval_error(
                "pow function requires runtime support, not yet implemented in Schema JIT"
                    .to_string(),
            ))
        }

        _ => Err(OrdoError::eval_error(format!(
            "Function '{}' not supported in Schema JIT",
            name
        ))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_schema_jit_compiler_creation() {
        let compiler = SchemaJITCompiler::new();
        assert!(compiler.is_ok());
    }

    #[test]
    fn test_can_compile_simple_expr() {
        let schema = MessageSchema::builder("TestContext")
            .field_at("amount", FieldType::Float64, 0)
            .field_at("count", FieldType::Int32, 8)
            .build();

        // Simple literal
        let expr = Expr::Literal(Value::Float(42.0));
        assert!(SchemaJITCompiler::can_compile_with_schema(&expr, &schema));

        // Field access
        let expr = Expr::Field("amount".to_string());
        assert!(SchemaJITCompiler::can_compile_with_schema(&expr, &schema));

        // Binary with fields
        let expr = Expr::Binary {
            left: Box::new(Expr::Field("amount".to_string())),
            op: BinaryOp::Gt,
            right: Box::new(Expr::Literal(Value::Float(100.0))),
        };
        assert!(SchemaJITCompiler::can_compile_with_schema(&expr, &schema));
    }

    #[test]
    fn test_cannot_compile_missing_field() {
        let schema = MessageSchema::builder("TestContext")
            .field_at("amount", FieldType::Float64, 0)
            .build();

        let expr = Expr::Field("nonexistent".to_string());
        assert!(!SchemaJITCompiler::can_compile_with_schema(&expr, &schema));
    }

    #[test]
    fn test_compile_and_execute() {
        #[repr(C)]
        struct TestContext {
            amount: f64,
            count: i32,
        }

        let mut compiler = SchemaJITCompiler::new().unwrap();

        let schema = MessageSchema::builder("TestContext")
            .field_at("amount", FieldType::Float64, 0)
            .field_at("count", FieldType::Int32, 8)
            .build();

        // Compile: amount * 2
        let expr = Expr::Binary {
            left: Box::new(Expr::Field("amount".to_string())),
            op: BinaryOp::Mul,
            right: Box::new(Expr::Literal(Value::Float(2.0))),
        };

        let compiled = compiler.compile_with_schema(&expr, 12345, &schema).unwrap();

        // Execute
        let ctx = TestContext {
            amount: 50.0,
            count: 10,
        };

        let result = unsafe { compiled.call_typed(&ctx).unwrap() };
        assert!((result - 100.0).abs() < 0.001);
    }

    #[test]
    fn test_compile_comparison() {
        #[repr(C)]
        struct TestContext {
            score: i32,
        }

        let mut compiler = SchemaJITCompiler::new().unwrap();

        let schema = MessageSchema::builder("TestContext")
            .field_at("score", FieldType::Int32, 0)
            .build();

        // Compile: score > 700
        let expr = Expr::Binary {
            left: Box::new(Expr::Field("score".to_string())),
            op: BinaryOp::Gt,
            right: Box::new(Expr::Literal(Value::Int(700))),
        };

        let compiled = compiler.compile_with_schema(&expr, 67890, &schema).unwrap();

        // Test with score = 750 (should be true = 1.0)
        let ctx1 = TestContext { score: 750 };
        let result1 = unsafe { compiled.call_typed(&ctx1).unwrap() };
        assert!((result1 - 1.0).abs() < 0.001);

        // Test with score = 650 (should be false = 0.0)
        let ctx2 = TestContext { score: 650 };
        let result2 = unsafe { compiled.call_typed(&ctx2).unwrap() };
        assert!(result2.abs() < 0.001);
    }

    #[test]
    fn test_compile_math_function() {
        #[repr(C)]
        struct TestContext {
            value: f64,
        }

        let mut compiler = SchemaJITCompiler::new().unwrap();

        let schema = MessageSchema::builder("TestContext")
            .field_at("value", FieldType::Float64, 0)
            .build();

        // Compile: abs(value)
        let expr = Expr::Call {
            name: "abs".to_string(),
            args: vec![Expr::Field("value".to_string())],
        };

        let compiled = compiler.compile_with_schema(&expr, 11111, &schema).unwrap();

        let ctx = TestContext { value: -42.5 };
        let result = unsafe { compiled.call_typed(&ctx).unwrap() };
        assert!((result - 42.5).abs() < 0.001);
    }

    #[test]
    fn test_stats() {
        let mut compiler = SchemaJITCompiler::new().unwrap();

        let schema = MessageSchema::builder("TestContext")
            .field_at("x", FieldType::Float64, 0)
            .build();

        let expr = Expr::Literal(Value::Float(1.0));
        compiler.compile_with_schema(&expr, 1, &schema).unwrap();

        let stats = compiler.stats();
        assert_eq!(stats.successful_compiles, 1);
        assert_eq!(stats.schema_compiles, 1);
        assert_eq!(stats.failed_compiles, 0);
    }
}
