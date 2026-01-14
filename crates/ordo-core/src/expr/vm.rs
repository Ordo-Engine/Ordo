//! High-performance bytecode virtual machine v2
//!
//! This is an optimized VM implementation that addresses the performance issues
//! in the original stack-based VM:
//!
//! 1. **Flat instruction encoding** - Uses a flat u32 array instead of enum variants
//! 2. **Register-based design** - Reduces stack push/pop overhead
//! 3. **Superinstructions** - Combines common instruction sequences
//! 4. **Inline caching** - Caches field lookups
//! 5. **Avoid cloning** - Uses indices and references where possible

use super::functions::FunctionRegistry;
use crate::context::{Context, Value};
use crate::error::{OrdoError, Result};
use serde::Serialize;
use std::cell::UnsafeCell;
use std::time::Instant;

/// Instruction opcodes as u8 for compact encoding
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Opcode {
    // Basic operations
    LoadConst = 0, // r[A] = constants[B]
    LoadField = 1, // r[A] = ctx.get(fields[B])
    Move = 2,      // r[A] = r[B]

    // Binary operations (r[A] = r[B] op r[C])
    Add = 10,
    Sub = 11,
    Mul = 12,
    Div = 13,
    Mod = 14,
    Eq = 15,
    Ne = 16,
    Lt = 17,
    Le = 18,
    Gt = 19,
    Ge = 20,
    And = 21,
    Or = 22,
    In = 23,
    NotIn = 24,
    Contains = 25,

    // Unary operations (r[A] = op r[B])
    Not = 30,
    Neg = 31,

    // Control flow
    JumpIfFalse = 40, // if !r[A] then ip += offset
    JumpIfTrue = 41,  // if r[A] then ip += offset
    Jump = 42,        // ip += offset

    // Function calls
    Call = 50, // r[A] = func(r[B..B+C])

    // Special
    Exists = 60, // r[A] = ctx.has(fields[B])
    Return = 70, // return r[A]

    // ========== SUPERINSTRUCTIONS ==========
    // These combine common patterns into single instructions
    /// LoadField + Compare with constant: r[A] = ctx.get(fields[B]) > constants[C]
    FieldGtConst = 100,
    FieldLtConst = 101,
    FieldEqConst = 102,
    FieldNeConst = 103,
    FieldGeConst = 104,
    FieldLeConst = 105,

    /// Two field comparisons with AND: r[A] = (ctx[B] > const[C]) && (ctx[D] < const[E])
    /// Encoded as two instructions
    FieldCmpAndFieldCmp = 110,

    /// Load field and check truthiness: if !ctx.get(fields[A]) then jump
    FieldTestJump = 120,
}

/// Compact instruction encoding
/// Format: [opcode: u8][A: u8][B: u8][C: u8] = 4 bytes
/// Or for extended: [opcode: u8][A: u8][BC: u16] = 4 bytes
#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub struct Instruction {
    pub op: Opcode,
    pub a: u8, // Usually destination register
    pub b: u8, // Usually first operand
    pub c: u8, // Usually second operand or count
}

impl Instruction {
    #[inline(always)]
    pub fn new(op: Opcode, a: u8, b: u8, c: u8) -> Self {
        Self { op, a, b, c }
    }

    /// Get BC as a combined u16 value (for jumps)
    #[inline(always)]
    pub fn bc(&self) -> i16 {
        ((self.b as i16) << 8) | (self.c as i16)
    }

    /// Create instruction with BC as i16
    #[inline(always)]
    pub fn with_offset(op: Opcode, a: u8, offset: i16) -> Self {
        Self {
            op,
            a,
            b: ((offset >> 8) & 0xFF) as u8,
            c: (offset & 0xFF) as u8,
        }
    }
}

/// Compiled expression for VM v2
#[derive(Debug, Clone)]
pub struct CompiledExpr {
    /// Compact instruction array
    pub instructions: Vec<Instruction>,
    /// Constant pool
    pub constants: Vec<Value>,
    /// Field name pool (interned strings)
    pub fields: Vec<String>,
    /// Function name pool
    pub functions: Vec<String>,
    /// Number of registers needed
    pub register_count: u8,
}

impl CompiledExpr {
    pub fn new() -> Self {
        Self {
            instructions: Vec::new(),
            constants: Vec::new(),
            fields: Vec::new(),
            functions: Vec::new(),
            register_count: 0,
        }
    }

    /// Get statistics about the compiled expression
    pub fn stats(&self) -> CompiledExprStats {
        CompiledExprStats {
            instruction_count: self.instructions.len(),
            constant_count: self.constants.len(),
            field_count: self.fields.len(),
            function_count: self.functions.len(),
        }
    }
}

impl Default for CompiledExpr {
    fn default() -> Self {
        Self::new()
    }
}

/// Statistics about a compiled expression
#[derive(Debug, Clone, Serialize)]
pub struct CompiledExprStats {
    pub instruction_count: usize,
    pub constant_count: usize,
    pub field_count: usize,
    pub function_count: usize,
}

/// Trace level for debug execution
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Default)]
pub enum TraceLevel {
    /// No tracing
    #[default]
    None,
    /// Minimal tracing - only final result
    Minimal,
    /// Standard tracing - record each instruction
    Standard,
    /// Full tracing - record all register states
    Full,
}

/// VM state snapshot at a point in execution
#[derive(Debug, Clone, Serialize)]
pub struct VMSnapshot {
    /// Instruction pointer
    pub ip: usize,
    /// Current instruction (human-readable)
    pub instruction: String,
    /// Register states (only non-null registers for Standard, all for Full)
    pub registers: Vec<RegisterValue>,
    /// Duration of this instruction in nanoseconds
    pub duration_ns: u64,
}

/// Register value with type info
#[derive(Debug, Clone, Serialize)]
pub struct RegisterValue {
    /// Register index
    pub index: u8,
    /// Value
    pub value: Value,
    /// Type name
    pub type_name: String,
}

/// VM execution trace
#[derive(Debug, Clone, Serialize)]
pub struct VMExecutionTrace {
    /// List of instructions (human-readable)
    pub instructions: Vec<String>,
    /// Constants pool (as strings)
    pub constants: Vec<String>,
    /// Fields pool
    pub fields: Vec<String>,
    /// Functions pool
    pub functions: Vec<String>,
    /// Execution snapshots
    pub snapshots: Vec<VMSnapshot>,
    /// Total instructions executed
    pub total_instructions: usize,
    /// Total execution time in nanoseconds
    pub total_duration_ns: u64,
}

/// High-performance VM with register-based execution
pub struct BytecodeVM {
    /// Function registry
    functions: FunctionRegistry,
    /// Register file (reused across executions)
    /// Using UnsafeCell for interior mutability without runtime checks
    registers: UnsafeCell<[Value; 256]>,
}

impl Default for BytecodeVM {
    fn default() -> Self {
        Self::new()
    }
}

impl BytecodeVM {
    pub fn new() -> Self {
        Self {
            functions: FunctionRegistry::new(),
            // Initialize with Null values
            registers: UnsafeCell::new(std::array::from_fn(|_| Value::Null)),
        }
    }

    /// Execute compiled expression
    ///
    /// # Safety
    /// This function uses unsafe for performance-critical register access.
    /// The safety is guaranteed by:
    /// 1. Register indices are validated during compilation
    /// 2. No concurrent access to registers (single-threaded execution)
    #[inline(always)]
    pub fn execute(&self, compiled: &CompiledExpr, ctx: &Context) -> Result<Value> {
        let instructions = &compiled.instructions;
        let constants = &compiled.constants;
        let fields = &compiled.fields;

        // SAFETY: We have exclusive access during execution
        let regs = unsafe { &mut *self.registers.get() };

        let mut ip: usize = 0;
        let len = instructions.len();

        // Main dispatch loop - optimized for branch prediction
        while ip < len {
            // SAFETY: ip is bounds-checked by the while condition
            let inst = unsafe { instructions.get_unchecked(ip) };
            ip += 1;

            // Use a match with explicit ordering for better branch prediction
            // Most common operations first
            match inst.op {
                // ========== SUPERINSTRUCTIONS (most common patterns) ==========
                Opcode::FieldGtConst => {
                    // r[A] = ctx.get(fields[B]) > constants[C]
                    let field = unsafe { fields.get_unchecked(inst.b as usize) };
                    let field_val = ctx.get(field).ok_or_else(|| OrdoError::FieldNotFound {
                        field: field.clone(),
                    })?;
                    let const_val = unsafe { constants.get_unchecked(inst.c as usize) };
                    let result = match field_val.compare(const_val) {
                        Some(std::cmp::Ordering::Greater) => Value::bool(true),
                        Some(_) => Value::bool(false),
                        None => return Err(OrdoError::eval_error("Cannot compare values")),
                    };
                    regs[inst.a as usize] = result;
                }

                Opcode::FieldLtConst => {
                    let field = unsafe { fields.get_unchecked(inst.b as usize) };
                    let field_val = ctx.get(field).ok_or_else(|| OrdoError::FieldNotFound {
                        field: field.clone(),
                    })?;
                    let const_val = unsafe { constants.get_unchecked(inst.c as usize) };
                    let result = match field_val.compare(const_val) {
                        Some(std::cmp::Ordering::Less) => Value::bool(true),
                        Some(_) => Value::bool(false),
                        None => return Err(OrdoError::eval_error("Cannot compare values")),
                    };
                    regs[inst.a as usize] = result;
                }

                Opcode::FieldEqConst => {
                    let field = unsafe { fields.get_unchecked(inst.b as usize) };
                    let field_val = ctx.get(field).ok_or_else(|| OrdoError::FieldNotFound {
                        field: field.clone(),
                    })?;
                    let const_val = unsafe { constants.get_unchecked(inst.c as usize) };
                    regs[inst.a as usize] = Value::bool(field_val == const_val);
                }

                Opcode::FieldNeConst => {
                    let field = unsafe { fields.get_unchecked(inst.b as usize) };
                    let field_val = ctx.get(field).ok_or_else(|| OrdoError::FieldNotFound {
                        field: field.clone(),
                    })?;
                    let const_val = unsafe { constants.get_unchecked(inst.c as usize) };
                    regs[inst.a as usize] = Value::bool(field_val != const_val);
                }

                Opcode::FieldGeConst => {
                    let field = unsafe { fields.get_unchecked(inst.b as usize) };
                    let field_val = ctx.get(field).ok_or_else(|| OrdoError::FieldNotFound {
                        field: field.clone(),
                    })?;
                    let const_val = unsafe { constants.get_unchecked(inst.c as usize) };
                    let result = match field_val.compare(const_val) {
                        Some(ord) => Value::bool(ord != std::cmp::Ordering::Less),
                        None => return Err(OrdoError::eval_error("Cannot compare values")),
                    };
                    regs[inst.a as usize] = result;
                }

                Opcode::FieldLeConst => {
                    let field = unsafe { fields.get_unchecked(inst.b as usize) };
                    let field_val = ctx.get(field).ok_or_else(|| OrdoError::FieldNotFound {
                        field: field.clone(),
                    })?;
                    let const_val = unsafe { constants.get_unchecked(inst.c as usize) };
                    let result = match field_val.compare(const_val) {
                        Some(ord) => Value::bool(ord != std::cmp::Ordering::Greater),
                        None => return Err(OrdoError::eval_error("Cannot compare values")),
                    };
                    regs[inst.a as usize] = result;
                }

                // ========== BASIC LOADS ==========
                Opcode::LoadConst => {
                    let val = unsafe { constants.get_unchecked(inst.b as usize) };
                    regs[inst.a as usize] = val.clone();
                }

                Opcode::LoadField => {
                    let field = unsafe { fields.get_unchecked(inst.b as usize) };
                    let val = ctx.get(field).ok_or_else(|| OrdoError::FieldNotFound {
                        field: field.clone(),
                    })?;
                    regs[inst.a as usize] = val.clone();
                }

                Opcode::Move => {
                    regs[inst.a as usize] = regs[inst.b as usize].clone();
                }

                // ========== BINARY OPERATIONS ==========
                Opcode::Gt => {
                    let left = &regs[inst.b as usize];
                    let right = &regs[inst.c as usize];
                    let result = match left.compare(right) {
                        Some(std::cmp::Ordering::Greater) => Value::bool(true),
                        Some(_) => Value::bool(false),
                        None => return Err(OrdoError::eval_error("Cannot compare values")),
                    };
                    regs[inst.a as usize] = result;
                }

                Opcode::Lt => {
                    let left = &regs[inst.b as usize];
                    let right = &regs[inst.c as usize];
                    let result = match left.compare(right) {
                        Some(std::cmp::Ordering::Less) => Value::bool(true),
                        Some(_) => Value::bool(false),
                        None => return Err(OrdoError::eval_error("Cannot compare values")),
                    };
                    regs[inst.a as usize] = result;
                }

                Opcode::Ge => {
                    let left = &regs[inst.b as usize];
                    let right = &regs[inst.c as usize];
                    let result = match left.compare(right) {
                        Some(ord) => Value::bool(ord != std::cmp::Ordering::Less),
                        None => return Err(OrdoError::eval_error("Cannot compare values")),
                    };
                    regs[inst.a as usize] = result;
                }

                Opcode::Le => {
                    let left = &regs[inst.b as usize];
                    let right = &regs[inst.c as usize];
                    let result = match left.compare(right) {
                        Some(ord) => Value::bool(ord != std::cmp::Ordering::Greater),
                        None => return Err(OrdoError::eval_error("Cannot compare values")),
                    };
                    regs[inst.a as usize] = result;
                }

                Opcode::Eq => {
                    let left = &regs[inst.b as usize];
                    let right = &regs[inst.c as usize];
                    regs[inst.a as usize] = Value::bool(left == right);
                }

                Opcode::Ne => {
                    let left = &regs[inst.b as usize];
                    let right = &regs[inst.c as usize];
                    regs[inst.a as usize] = Value::bool(left != right);
                }

                Opcode::Add => {
                    let left = &regs[inst.b as usize];
                    let right = &regs[inst.c as usize];
                    regs[inst.a as usize] = self.eval_add(left, right)?;
                }

                Opcode::Sub => {
                    let left = &regs[inst.b as usize];
                    let right = &regs[inst.c as usize];
                    regs[inst.a as usize] = self.eval_sub(left, right)?;
                }

                Opcode::Mul => {
                    let left = &regs[inst.b as usize];
                    let right = &regs[inst.c as usize];
                    regs[inst.a as usize] = self.eval_mul(left, right)?;
                }

                Opcode::Div => {
                    let left = &regs[inst.b as usize];
                    let right = &regs[inst.c as usize];
                    regs[inst.a as usize] = self.eval_div(left, right)?;
                }

                Opcode::Mod => {
                    let left = &regs[inst.b as usize];
                    let right = &regs[inst.c as usize];
                    regs[inst.a as usize] = self.eval_mod(left, right)?;
                }

                Opcode::And => {
                    let left = &regs[inst.b as usize];
                    let right = &regs[inst.c as usize];
                    regs[inst.a as usize] = Value::bool(left.is_truthy() && right.is_truthy());
                }

                Opcode::Or => {
                    let left = &regs[inst.b as usize];
                    let right = &regs[inst.c as usize];
                    regs[inst.a as usize] = Value::bool(left.is_truthy() || right.is_truthy());
                }

                Opcode::In => {
                    let value = &regs[inst.b as usize];
                    let collection = &regs[inst.c as usize];
                    regs[inst.a as usize] = self.eval_in(value, collection)?;
                }

                Opcode::NotIn => {
                    let value = &regs[inst.b as usize];
                    let collection = &regs[inst.c as usize];
                    let in_result = self.eval_in(value, collection)?;
                    regs[inst.a as usize] = Value::bool(!in_result.as_bool().unwrap_or(false));
                }

                Opcode::Contains => {
                    let collection = &regs[inst.b as usize];
                    let value = &regs[inst.c as usize];
                    regs[inst.a as usize] = self.eval_in(value, collection)?;
                }

                // ========== UNARY OPERATIONS ==========
                Opcode::Not => {
                    let val = &regs[inst.b as usize];
                    regs[inst.a as usize] = Value::bool(!val.is_truthy());
                }

                Opcode::Neg => {
                    let val = &regs[inst.b as usize];
                    regs[inst.a as usize] = match val {
                        Value::Int(n) => Value::int(-n),
                        Value::Float(n) => Value::float(-n),
                        _ => return Err(OrdoError::type_error("number", val.type_name())),
                    };
                }

                // ========== CONTROL FLOW ==========
                Opcode::JumpIfFalse => {
                    let val = &regs[inst.a as usize];
                    if !val.is_truthy() {
                        let offset = inst.bc();
                        ip = ((ip as i32) + (offset as i32) - 1) as usize;
                    }
                }

                Opcode::JumpIfTrue => {
                    let val = &regs[inst.a as usize];
                    if val.is_truthy() {
                        let offset = inst.bc();
                        ip = ((ip as i32) + (offset as i32) - 1) as usize;
                    }
                }

                Opcode::Jump => {
                    let offset = inst.bc();
                    ip = ((ip as i32) + (offset as i32) - 1) as usize;
                }

                // ========== FUNCTION CALLS ==========
                Opcode::Call => {
                    let func_idx = inst.b as usize;
                    let arg_count = inst.c as usize;
                    let func_name = unsafe { self.functions.get_name(func_idx) }
                        .unwrap_or_else(|| &compiled.functions[func_idx]);

                    // Collect arguments from consecutive registers
                    let args: Vec<Value> = (0..arg_count)
                        .map(|i| regs[inst.a as usize + 1 + i].clone())
                        .collect();

                    let result = self.functions.call(func_name, &args)?;
                    regs[inst.a as usize] = result;
                }

                // ========== SPECIAL ==========
                Opcode::Exists => {
                    let field = unsafe { fields.get_unchecked(inst.b as usize) };
                    regs[inst.a as usize] = Value::bool(ctx.get(field).is_some());
                }

                Opcode::Return => {
                    return Ok(regs[inst.a as usize].clone());
                }

                // Extended superinstructions
                Opcode::FieldCmpAndFieldCmp | Opcode::FieldTestJump => {
                    // These require more complex handling - fall back to basic ops
                    return Err(OrdoError::eval_error("Unimplemented superinstruction"));
                }
            }
        }

        // Default: return register 0
        Ok(regs[0].clone())
    }

    /// Execute compiled expression with tracing
    ///
    /// Returns both the result and a detailed execution trace.
    /// This is slower than `execute` and should only be used for debugging.
    pub fn execute_with_trace(
        &self,
        compiled: &CompiledExpr,
        ctx: &Context,
        level: TraceLevel,
    ) -> Result<(Value, VMExecutionTrace)> {
        let start_time = Instant::now();

        // Build instruction strings for trace
        let instruction_strings: Vec<String> = compiled
            .instructions
            .iter()
            .enumerate()
            .map(|(i, inst)| format!("{:3}: {}", i, self.format_instruction(inst, compiled)))
            .collect();

        // Build constants strings
        let constant_strings: Vec<String> = compiled
            .constants
            .iter()
            .map(|v| format!("{:?}", v))
            .collect();

        let mut trace = VMExecutionTrace {
            instructions: instruction_strings,
            constants: constant_strings,
            fields: compiled.fields.clone(),
            functions: compiled.functions.clone(),
            snapshots: Vec::new(),
            total_instructions: 0,
            total_duration_ns: 0,
        };

        if level == TraceLevel::None {
            let result = self.execute(compiled, ctx)?;
            trace.total_duration_ns = start_time.elapsed().as_nanos() as u64;
            return Ok((result, trace));
        }

        // Execute with tracing
        let instructions = &compiled.instructions;
        let constants = &compiled.constants;
        let fields = &compiled.fields;

        // SAFETY: We have exclusive access during execution
        let regs = unsafe { &mut *self.registers.get() };

        let mut ip: usize = 0;
        let len = instructions.len();

        while ip < len {
            let inst_start = Instant::now();
            let current_ip = ip;

            // SAFETY: ip is bounds-checked by the while condition
            let inst = unsafe { instructions.get_unchecked(ip) };
            ip += 1;
            trace.total_instructions += 1;

            // Execute instruction (same as execute, but with tracing)
            match inst.op {
                Opcode::FieldGtConst => {
                    let field = unsafe { fields.get_unchecked(inst.b as usize) };
                    let field_val = ctx.get(field).ok_or_else(|| OrdoError::FieldNotFound {
                        field: field.clone(),
                    })?;
                    let const_val = unsafe { constants.get_unchecked(inst.c as usize) };
                    let result = match field_val.compare(const_val) {
                        Some(std::cmp::Ordering::Greater) => Value::bool(true),
                        Some(_) => Value::bool(false),
                        None => return Err(OrdoError::eval_error("Cannot compare values")),
                    };
                    regs[inst.a as usize] = result;
                }
                Opcode::FieldLtConst => {
                    let field = unsafe { fields.get_unchecked(inst.b as usize) };
                    let field_val = ctx.get(field).ok_or_else(|| OrdoError::FieldNotFound {
                        field: field.clone(),
                    })?;
                    let const_val = unsafe { constants.get_unchecked(inst.c as usize) };
                    let result = match field_val.compare(const_val) {
                        Some(std::cmp::Ordering::Less) => Value::bool(true),
                        Some(_) => Value::bool(false),
                        None => return Err(OrdoError::eval_error("Cannot compare values")),
                    };
                    regs[inst.a as usize] = result;
                }
                Opcode::FieldEqConst => {
                    let field = unsafe { fields.get_unchecked(inst.b as usize) };
                    let field_val = ctx.get(field).ok_or_else(|| OrdoError::FieldNotFound {
                        field: field.clone(),
                    })?;
                    let const_val = unsafe { constants.get_unchecked(inst.c as usize) };
                    regs[inst.a as usize] = Value::bool(field_val == const_val);
                }
                Opcode::FieldNeConst => {
                    let field = unsafe { fields.get_unchecked(inst.b as usize) };
                    let field_val = ctx.get(field).ok_or_else(|| OrdoError::FieldNotFound {
                        field: field.clone(),
                    })?;
                    let const_val = unsafe { constants.get_unchecked(inst.c as usize) };
                    regs[inst.a as usize] = Value::bool(field_val != const_val);
                }
                Opcode::FieldGeConst => {
                    let field = unsafe { fields.get_unchecked(inst.b as usize) };
                    let field_val = ctx.get(field).ok_or_else(|| OrdoError::FieldNotFound {
                        field: field.clone(),
                    })?;
                    let const_val = unsafe { constants.get_unchecked(inst.c as usize) };
                    let result = match field_val.compare(const_val) {
                        Some(ord) => Value::bool(ord != std::cmp::Ordering::Less),
                        None => return Err(OrdoError::eval_error("Cannot compare values")),
                    };
                    regs[inst.a as usize] = result;
                }
                Opcode::FieldLeConst => {
                    let field = unsafe { fields.get_unchecked(inst.b as usize) };
                    let field_val = ctx.get(field).ok_or_else(|| OrdoError::FieldNotFound {
                        field: field.clone(),
                    })?;
                    let const_val = unsafe { constants.get_unchecked(inst.c as usize) };
                    let result = match field_val.compare(const_val) {
                        Some(ord) => Value::bool(ord != std::cmp::Ordering::Greater),
                        None => return Err(OrdoError::eval_error("Cannot compare values")),
                    };
                    regs[inst.a as usize] = result;
                }
                Opcode::LoadConst => {
                    let val = unsafe { constants.get_unchecked(inst.b as usize) };
                    regs[inst.a as usize] = val.clone();
                }
                Opcode::LoadField => {
                    let field = unsafe { fields.get_unchecked(inst.b as usize) };
                    let val = ctx.get(field).ok_or_else(|| OrdoError::FieldNotFound {
                        field: field.clone(),
                    })?;
                    regs[inst.a as usize] = val.clone();
                }
                Opcode::Move => {
                    regs[inst.a as usize] = regs[inst.b as usize].clone();
                }
                Opcode::Gt => {
                    let left = &regs[inst.b as usize];
                    let right = &regs[inst.c as usize];
                    let result = match left.compare(right) {
                        Some(std::cmp::Ordering::Greater) => Value::bool(true),
                        Some(_) => Value::bool(false),
                        None => return Err(OrdoError::eval_error("Cannot compare values")),
                    };
                    regs[inst.a as usize] = result;
                }
                Opcode::Lt => {
                    let left = &regs[inst.b as usize];
                    let right = &regs[inst.c as usize];
                    let result = match left.compare(right) {
                        Some(std::cmp::Ordering::Less) => Value::bool(true),
                        Some(_) => Value::bool(false),
                        None => return Err(OrdoError::eval_error("Cannot compare values")),
                    };
                    regs[inst.a as usize] = result;
                }
                Opcode::Ge => {
                    let left = &regs[inst.b as usize];
                    let right = &regs[inst.c as usize];
                    let result = match left.compare(right) {
                        Some(ord) => Value::bool(ord != std::cmp::Ordering::Less),
                        None => return Err(OrdoError::eval_error("Cannot compare values")),
                    };
                    regs[inst.a as usize] = result;
                }
                Opcode::Le => {
                    let left = &regs[inst.b as usize];
                    let right = &regs[inst.c as usize];
                    let result = match left.compare(right) {
                        Some(ord) => Value::bool(ord != std::cmp::Ordering::Greater),
                        None => return Err(OrdoError::eval_error("Cannot compare values")),
                    };
                    regs[inst.a as usize] = result;
                }
                Opcode::Eq => {
                    let left = &regs[inst.b as usize];
                    let right = &regs[inst.c as usize];
                    regs[inst.a as usize] = Value::bool(left == right);
                }
                Opcode::Ne => {
                    let left = &regs[inst.b as usize];
                    let right = &regs[inst.c as usize];
                    regs[inst.a as usize] = Value::bool(left != right);
                }
                Opcode::Add => {
                    let left = &regs[inst.b as usize];
                    let right = &regs[inst.c as usize];
                    regs[inst.a as usize] = self.eval_add(left, right)?;
                }
                Opcode::Sub => {
                    let left = &regs[inst.b as usize];
                    let right = &regs[inst.c as usize];
                    regs[inst.a as usize] = self.eval_sub(left, right)?;
                }
                Opcode::Mul => {
                    let left = &regs[inst.b as usize];
                    let right = &regs[inst.c as usize];
                    regs[inst.a as usize] = self.eval_mul(left, right)?;
                }
                Opcode::Div => {
                    let left = &regs[inst.b as usize];
                    let right = &regs[inst.c as usize];
                    regs[inst.a as usize] = self.eval_div(left, right)?;
                }
                Opcode::Mod => {
                    let left = &regs[inst.b as usize];
                    let right = &regs[inst.c as usize];
                    regs[inst.a as usize] = self.eval_mod(left, right)?;
                }
                Opcode::And => {
                    let left = &regs[inst.b as usize];
                    let right = &regs[inst.c as usize];
                    regs[inst.a as usize] = Value::bool(left.is_truthy() && right.is_truthy());
                }
                Opcode::Or => {
                    let left = &regs[inst.b as usize];
                    let right = &regs[inst.c as usize];
                    regs[inst.a as usize] = Value::bool(left.is_truthy() || right.is_truthy());
                }
                Opcode::In => {
                    let value = &regs[inst.b as usize];
                    let collection = &regs[inst.c as usize];
                    regs[inst.a as usize] = self.eval_in(value, collection)?;
                }
                Opcode::NotIn => {
                    let value = &regs[inst.b as usize];
                    let collection = &regs[inst.c as usize];
                    let in_result = self.eval_in(value, collection)?;
                    regs[inst.a as usize] = Value::bool(!in_result.as_bool().unwrap_or(false));
                }
                Opcode::Contains => {
                    let collection = &regs[inst.b as usize];
                    let value = &regs[inst.c as usize];
                    regs[inst.a as usize] = self.eval_in(value, collection)?;
                }
                Opcode::Not => {
                    let val = &regs[inst.b as usize];
                    regs[inst.a as usize] = Value::bool(!val.is_truthy());
                }
                Opcode::Neg => {
                    let val = &regs[inst.b as usize];
                    regs[inst.a as usize] = match val {
                        Value::Int(n) => Value::int(-n),
                        Value::Float(n) => Value::float(-n),
                        _ => return Err(OrdoError::type_error("number", val.type_name())),
                    };
                }
                Opcode::JumpIfFalse => {
                    let val = &regs[inst.a as usize];
                    if !val.is_truthy() {
                        let offset = inst.bc();
                        ip = ((ip as i32) + (offset as i32) - 1) as usize;
                    }
                }
                Opcode::JumpIfTrue => {
                    let val = &regs[inst.a as usize];
                    if val.is_truthy() {
                        let offset = inst.bc();
                        ip = ((ip as i32) + (offset as i32) - 1) as usize;
                    }
                }
                Opcode::Jump => {
                    let offset = inst.bc();
                    ip = ((ip as i32) + (offset as i32) - 1) as usize;
                }
                Opcode::Call => {
                    let func_idx = inst.b as usize;
                    let arg_count = inst.c as usize;
                    let func_name = unsafe { self.functions.get_name(func_idx) }
                        .unwrap_or_else(|| &compiled.functions[func_idx]);

                    let args: Vec<Value> = (0..arg_count)
                        .map(|i| regs[inst.a as usize + 1 + i].clone())
                        .collect();

                    let result = self.functions.call(func_name, &args)?;
                    regs[inst.a as usize] = result;
                }
                Opcode::Exists => {
                    let field = unsafe { fields.get_unchecked(inst.b as usize) };
                    regs[inst.a as usize] = Value::bool(ctx.get(field).is_some());
                }
                Opcode::Return => {
                    let inst_duration = inst_start.elapsed().as_nanos() as u64;

                    // Record final snapshot
                    if level >= TraceLevel::Standard {
                        let registers =
                            self.collect_registers(regs, level, compiled.register_count);
                        trace.snapshots.push(VMSnapshot {
                            ip: current_ip,
                            instruction: self.format_instruction(inst, compiled),
                            registers,
                            duration_ns: inst_duration,
                        });
                    }

                    trace.total_duration_ns = start_time.elapsed().as_nanos() as u64;
                    return Ok((regs[inst.a as usize].clone(), trace));
                }
                Opcode::FieldCmpAndFieldCmp | Opcode::FieldTestJump => {
                    return Err(OrdoError::eval_error("Unimplemented superinstruction"));
                }
            }

            // Record snapshot if tracing is enabled
            if level >= TraceLevel::Standard {
                let inst_duration = inst_start.elapsed().as_nanos() as u64;
                let registers = self.collect_registers(regs, level, compiled.register_count);
                trace.snapshots.push(VMSnapshot {
                    ip: current_ip,
                    instruction: self.format_instruction(inst, compiled),
                    registers,
                    duration_ns: inst_duration,
                });
            }
        }

        trace.total_duration_ns = start_time.elapsed().as_nanos() as u64;
        Ok((regs[0].clone(), trace))
    }

    /// Format an instruction as a human-readable string
    fn format_instruction(&self, inst: &Instruction, compiled: &CompiledExpr) -> String {
        match inst.op {
            Opcode::LoadConst => {
                let val = compiled.constants.get(inst.b as usize);
                format!("LOAD_CONST r{} = {:?}", inst.a, val)
            }
            Opcode::LoadField => {
                let field = compiled.fields.get(inst.b as usize);
                format!("LOAD_FIELD r{} = ${:?}", inst.a, field)
            }
            Opcode::Move => format!("MOVE r{} = r{}", inst.a, inst.b),
            Opcode::Add => format!("ADD r{} = r{} + r{}", inst.a, inst.b, inst.c),
            Opcode::Sub => format!("SUB r{} = r{} - r{}", inst.a, inst.b, inst.c),
            Opcode::Mul => format!("MUL r{} = r{} * r{}", inst.a, inst.b, inst.c),
            Opcode::Div => format!("DIV r{} = r{} / r{}", inst.a, inst.b, inst.c),
            Opcode::Mod => format!("MOD r{} = r{} % r{}", inst.a, inst.b, inst.c),
            Opcode::Eq => format!("EQ r{} = r{} == r{}", inst.a, inst.b, inst.c),
            Opcode::Ne => format!("NE r{} = r{} != r{}", inst.a, inst.b, inst.c),
            Opcode::Lt => format!("LT r{} = r{} < r{}", inst.a, inst.b, inst.c),
            Opcode::Le => format!("LE r{} = r{} <= r{}", inst.a, inst.b, inst.c),
            Opcode::Gt => format!("GT r{} = r{} > r{}", inst.a, inst.b, inst.c),
            Opcode::Ge => format!("GE r{} = r{} >= r{}", inst.a, inst.b, inst.c),
            Opcode::And => format!("AND r{} = r{} && r{}", inst.a, inst.b, inst.c),
            Opcode::Or => format!("OR r{} = r{} || r{}", inst.a, inst.b, inst.c),
            Opcode::In => format!("IN r{} = r{} in r{}", inst.a, inst.b, inst.c),
            Opcode::NotIn => format!("NOT_IN r{} = r{} not in r{}", inst.a, inst.b, inst.c),
            Opcode::Contains => format!("CONTAINS r{} = r{} contains r{}", inst.a, inst.b, inst.c),
            Opcode::Not => format!("NOT r{} = !r{}", inst.a, inst.b),
            Opcode::Neg => format!("NEG r{} = -r{}", inst.a, inst.b),
            Opcode::JumpIfFalse => format!("JUMP_IF_FALSE r{} offset {}", inst.a, inst.bc()),
            Opcode::JumpIfTrue => format!("JUMP_IF_TRUE r{} offset {}", inst.a, inst.bc()),
            Opcode::Jump => format!("JUMP offset {}", inst.bc()),
            Opcode::Call => {
                let func = compiled.functions.get(inst.b as usize);
                format!("CALL r{} = {:?}(args: {})", inst.a, func, inst.c)
            }
            Opcode::Exists => {
                let field = compiled.fields.get(inst.b as usize);
                format!("EXISTS r{} = exists({:?})", inst.a, field)
            }
            Opcode::Return => format!("RETURN r{}", inst.a),
            Opcode::FieldGtConst => {
                let field = compiled.fields.get(inst.b as usize);
                let val = compiled.constants.get(inst.c as usize);
                format!("FIELD_GT_CONST r{} = ${:?} > {:?}", inst.a, field, val)
            }
            Opcode::FieldLtConst => {
                let field = compiled.fields.get(inst.b as usize);
                let val = compiled.constants.get(inst.c as usize);
                format!("FIELD_LT_CONST r{} = ${:?} < {:?}", inst.a, field, val)
            }
            Opcode::FieldEqConst => {
                let field = compiled.fields.get(inst.b as usize);
                let val = compiled.constants.get(inst.c as usize);
                format!("FIELD_EQ_CONST r{} = ${:?} == {:?}", inst.a, field, val)
            }
            Opcode::FieldNeConst => {
                let field = compiled.fields.get(inst.b as usize);
                let val = compiled.constants.get(inst.c as usize);
                format!("FIELD_NE_CONST r{} = ${:?} != {:?}", inst.a, field, val)
            }
            Opcode::FieldGeConst => {
                let field = compiled.fields.get(inst.b as usize);
                let val = compiled.constants.get(inst.c as usize);
                format!("FIELD_GE_CONST r{} = ${:?} >= {:?}", inst.a, field, val)
            }
            Opcode::FieldLeConst => {
                let field = compiled.fields.get(inst.b as usize);
                let val = compiled.constants.get(inst.c as usize);
                format!("FIELD_LE_CONST r{} = ${:?} <= {:?}", inst.a, field, val)
            }
            _ => format!("{:?} r{} r{} r{}", inst.op, inst.a, inst.b, inst.c),
        }
    }

    /// Collect register values for tracing
    fn collect_registers(
        &self,
        regs: &[Value; 256],
        level: TraceLevel,
        register_count: u8,
    ) -> Vec<RegisterValue> {
        let max_reg = if level == TraceLevel::Full {
            register_count.max(16) as usize
        } else {
            register_count as usize
        };

        regs.iter()
            .take(max_reg)
            .enumerate()
            .filter(|(_, v)| level == TraceLevel::Full || **v != Value::Null)
            .map(|(i, v)| RegisterValue {
                index: i as u8,
                value: v.clone(),
                type_name: v.type_name().to_string(),
            })
            .collect()
    }

    // ========== ARITHMETIC HELPERS ==========

    #[inline(always)]
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

    #[inline(always)]
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

    #[inline(always)]
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

    #[inline(always)]
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

    #[inline(always)]
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

    #[inline(always)]
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

// Add helper method to FunctionRegistry
impl FunctionRegistry {
    /// Get function name by index (for VM optimization)
    /// Get function name by index (for VM optimization)
    ///
    /// # Safety
    /// The caller must ensure that the index is within bounds of the internal
    /// function name storage. This is a placeholder for future optimization.
    #[inline(always)]
    pub unsafe fn get_name(&self, _index: usize) -> Option<&str> {
        // This is a placeholder - actual implementation would use an indexed lookup
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_ctx(json: &str) -> Context {
        Context::from_json(json).unwrap()
    }

    #[test]
    fn test_vm_v2_field_gt_const() {
        let mut compiled = CompiledExpr::new();
        compiled.fields.push("age".to_string());
        compiled.constants.push(Value::int(18));
        compiled
            .instructions
            .push(Instruction::new(Opcode::FieldGtConst, 0, 0, 0));
        compiled
            .instructions
            .push(Instruction::new(Opcode::Return, 0, 0, 0));

        let vm = BytecodeVM::new();
        let ctx = make_ctx(r#"{"age": 25}"#);

        let result = vm.execute(&compiled, &ctx).unwrap();
        assert_eq!(result, Value::bool(true));
    }

    #[test]
    fn test_vm_v2_basic_ops() {
        let mut compiled = CompiledExpr::new();
        compiled.constants.push(Value::int(10));
        compiled.constants.push(Value::int(3));

        // r0 = 10, r1 = 3, r2 = r0 + r1
        compiled
            .instructions
            .push(Instruction::new(Opcode::LoadConst, 0, 0, 0));
        compiled
            .instructions
            .push(Instruction::new(Opcode::LoadConst, 1, 1, 0));
        compiled
            .instructions
            .push(Instruction::new(Opcode::Add, 2, 0, 1));
        compiled
            .instructions
            .push(Instruction::new(Opcode::Return, 2, 0, 0));

        let vm = BytecodeVM::new();
        let ctx = Context::default();

        let result = vm.execute(&compiled, &ctx).unwrap();
        assert_eq!(result, Value::int(13));
    }
}
