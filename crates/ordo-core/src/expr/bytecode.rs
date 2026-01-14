//! Bytecode definitions for the expression virtual machine
//!
//! This module defines the bytecode instruction set used by the VM to execute
//! compiled expressions. The bytecode is designed to be:
//! - Compact: instructions are small and cache-friendly
//! - Fast: common operations are single instructions
//! - Safe: all operations are bounds-checked

use super::ast::{BinaryOp, UnaryOp};
use crate::context::Value;

/// Bytecode instruction
#[derive(Debug, Clone, PartialEq)]
pub enum Opcode {
    /// Push a constant value onto the stack
    /// Index into the constant pool
    LoadConst(u16),

    /// Load a field value from the context onto the stack
    /// Index into the field name pool
    LoadField(u16),

    /// Perform a binary operation
    /// Pops two values, pushes result
    BinaryOp(BinaryOp),

    /// Perform a unary operation
    /// Pops one value, pushes result
    UnaryOp(UnaryOp),

    /// Call a built-in function
    /// (function name index, argument count)
    /// Pops `arg_count` values, pushes result
    Call(u16, u8),

    /// Jump forward by offset if top of stack is falsy (short-circuit AND)
    /// Does NOT pop the condition value
    JumpIfFalse(i16),

    /// Jump forward by offset if top of stack is truthy (short-circuit OR)
    /// Does NOT pop the condition value
    JumpIfTrue(i16),

    /// Unconditional jump
    Jump(i16),

    /// Pop the top value from the stack (used after short-circuit jumps)
    Pop,

    /// Check if a field exists in the context
    /// Index into the field name pool
    Exists(u16),

    /// Create an array from top N stack values
    MakeArray(u16),

    /// Create an object from top N*2 stack values (key-value pairs)
    /// Keys are loaded as constants before this instruction
    MakeObject(u16),

    /// Duplicate the top value on the stack
    Dup,

    /// Return the top value on the stack
    Return,
}

/// Compiled bytecode for an expression
#[derive(Debug, Clone)]
pub struct CompiledExpr {
    /// The bytecode instructions
    pub instructions: Vec<Opcode>,

    /// Constant pool (literal values)
    pub constants: Vec<Value>,

    /// Field name pool (field paths)
    pub fields: Vec<String>,

    /// Function name pool
    pub functions: Vec<String>,
}

impl CompiledExpr {
    /// Create a new empty compiled expression
    pub fn new() -> Self {
        Self {
            instructions: Vec::new(),
            constants: Vec::new(),
            fields: Vec::new(),
            functions: Vec::new(),
        }
    }

    /// Add a constant to the pool, returning its index
    pub fn add_constant(&mut self, value: Value) -> u16 {
        // Check if constant already exists
        if let Some(idx) = self.constants.iter().position(|v| v == &value) {
            return idx as u16;
        }
        let idx = self.constants.len();
        self.constants.push(value);
        idx as u16
    }

    /// Add a field name to the pool, returning its index
    pub fn add_field(&mut self, name: String) -> u16 {
        // Check if field already exists
        if let Some(idx) = self.fields.iter().position(|f| f == &name) {
            return idx as u16;
        }
        let idx = self.fields.len();
        self.fields.push(name);
        idx as u16
    }

    /// Add a function name to the pool, returning its index
    pub fn add_function(&mut self, name: String) -> u16 {
        // Check if function already exists
        if let Some(idx) = self.functions.iter().position(|f| f == &name) {
            return idx as u16;
        }
        let idx = self.functions.len();
        self.functions.push(name);
        idx as u16
    }

    /// Add an instruction
    pub fn emit(&mut self, op: Opcode) {
        self.instructions.push(op);
    }

    /// Get the current instruction offset
    pub fn current_offset(&self) -> usize {
        self.instructions.len()
    }

    /// Patch a jump instruction at the given offset
    pub fn patch_jump(&mut self, offset: usize, target: i16) {
        match &mut self.instructions[offset] {
            Opcode::JumpIfFalse(ref mut delta) => *delta = target,
            Opcode::JumpIfTrue(ref mut delta) => *delta = target,
            Opcode::Jump(ref mut delta) => *delta = target,
            _ => panic!("Cannot patch non-jump instruction"),
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
#[derive(Debug, Clone)]
pub struct CompiledExprStats {
    pub instruction_count: usize,
    pub constant_count: usize,
    pub field_count: usize,
    pub function_count: usize,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compiled_expr_constants() {
        let mut expr = CompiledExpr::new();

        let idx1 = expr.add_constant(Value::int(42));
        let idx2 = expr.add_constant(Value::string("hello"));
        let idx3 = expr.add_constant(Value::int(42)); // duplicate

        assert_eq!(idx1, 0);
        assert_eq!(idx2, 1);
        assert_eq!(idx3, 0); // should reuse
        assert_eq!(expr.constants.len(), 2);
    }

    #[test]
    fn test_compiled_expr_fields() {
        let mut expr = CompiledExpr::new();

        let idx1 = expr.add_field("user.name".to_string());
        let idx2 = expr.add_field("user.age".to_string());
        let idx3 = expr.add_field("user.name".to_string()); // duplicate

        assert_eq!(idx1, 0);
        assert_eq!(idx2, 1);
        assert_eq!(idx3, 0); // should reuse
        assert_eq!(expr.fields.len(), 2);
    }

    #[test]
    fn test_compiled_expr_emit() {
        let mut expr = CompiledExpr::new();

        let const_idx = expr.add_constant(Value::int(42));
        expr.emit(Opcode::LoadConst(const_idx));
        expr.emit(Opcode::Return);

        assert_eq!(expr.instructions.len(), 2);
        assert_eq!(expr.instructions[0], Opcode::LoadConst(0));
        assert_eq!(expr.instructions[1], Opcode::Return);
    }

    #[test]
    fn test_jump_patching() {
        let mut expr = CompiledExpr::new();

        expr.emit(Opcode::JumpIfFalse(0)); // placeholder
        let jump_offset = 0;
        expr.emit(Opcode::LoadConst(0));
        expr.emit(Opcode::Return);

        // Patch the jump to skip to Return
        expr.patch_jump(jump_offset, 2);

        assert_eq!(expr.instructions[0], Opcode::JumpIfFalse(2));
    }
}
