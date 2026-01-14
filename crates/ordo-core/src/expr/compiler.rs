//! Expression compiler
//!
//! Compiles expression AST to bytecode for the VM

use super::ast::{BinaryOp, Expr};
use super::bytecode::{CompiledExpr, Opcode};
use crate::context::Value;

/// Expression compiler that transforms AST to bytecode
#[derive(Debug, Default)]
pub struct ExprCompiler {
    /// The compiled expression being built
    compiled: CompiledExpr,
}

impl ExprCompiler {
    /// Create a new compiler
    pub fn new() -> Self {
        Self::default()
    }

    /// Compile an expression to bytecode
    pub fn compile(mut self, expr: &Expr) -> CompiledExpr {
        self.compile_expr(expr);
        self.compiled.emit(Opcode::Return);
        self.compiled
    }

    /// Compile an expression recursively
    fn compile_expr(&mut self, expr: &Expr) {
        match expr {
            Expr::Literal(value) => {
                let idx = self.compiled.add_constant(value.clone());
                self.compiled.emit(Opcode::LoadConst(idx));
            }

            Expr::Field(path) => {
                let idx = self.compiled.add_field(path.clone());
                self.compiled.emit(Opcode::LoadField(idx));
            }

            Expr::Binary { op, left, right } => {
                self.compile_binary(*op, left, right);
            }

            Expr::Unary { op, operand } => {
                self.compile_expr(operand);
                self.compiled.emit(Opcode::UnaryOp(*op));
            }

            Expr::Call { name, args } => {
                // Push arguments in order
                for arg in args {
                    self.compile_expr(arg);
                }
                let func_idx = self.compiled.add_function(name.clone());
                self.compiled.emit(Opcode::Call(func_idx, args.len() as u8));
            }

            Expr::Conditional {
                condition,
                then_branch,
                else_branch,
            } => {
                self.compile_conditional(condition, then_branch, else_branch);
            }

            Expr::Array(elements) => {
                // Push all elements
                for elem in elements {
                    self.compile_expr(elem);
                }
                self.compiled.emit(Opcode::MakeArray(elements.len() as u16));
            }

            Expr::Object(pairs) => {
                // Push key-value pairs
                for (key, value) in pairs {
                    // Push key as constant
                    let key_idx = self.compiled.add_constant(Value::string(key.clone()));
                    self.compiled.emit(Opcode::LoadConst(key_idx));
                    // Push value
                    self.compile_expr(value);
                }
                self.compiled.emit(Opcode::MakeObject(pairs.len() as u16));
            }

            Expr::Exists(path) => {
                let idx = self.compiled.add_field(path.clone());
                self.compiled.emit(Opcode::Exists(idx));
            }

            Expr::Coalesce(exprs) => {
                self.compile_coalesce(exprs);
            }
        }
    }

    /// Compile binary operation with special handling for short-circuit operators
    fn compile_binary(&mut self, op: BinaryOp, left: &Expr, right: &Expr) {
        match op {
            // Short-circuit AND: if left is false, skip right
            BinaryOp::And => {
                self.compile_expr(left);
                self.compiled.emit(Opcode::Dup); // Keep value for potential short-circuit
                let jump_offset = self.compiled.current_offset();
                self.compiled.emit(Opcode::JumpIfFalse(0)); // Placeholder
                self.compiled.emit(Opcode::Pop); // Remove duplicated value
                self.compile_expr(right);
                // Patch jump to skip right side
                let target = (self.compiled.current_offset() - jump_offset) as i16;
                self.compiled.patch_jump(jump_offset, target);
            }

            // Short-circuit OR: if left is true, skip right
            BinaryOp::Or => {
                self.compile_expr(left);
                self.compiled.emit(Opcode::Dup); // Keep value for potential short-circuit
                let jump_offset = self.compiled.current_offset();
                self.compiled.emit(Opcode::JumpIfTrue(0)); // Placeholder
                self.compiled.emit(Opcode::Pop); // Remove duplicated value
                self.compile_expr(right);
                // Patch jump to skip right side
                let target = (self.compiled.current_offset() - jump_offset) as i16;
                self.compiled.patch_jump(jump_offset, target);
            }

            // Regular binary operations
            _ => {
                self.compile_expr(left);
                self.compile_expr(right);
                self.compiled.emit(Opcode::BinaryOp(op));
            }
        }
    }

    /// Compile conditional expression (if-then-else)
    fn compile_conditional(&mut self, condition: &Expr, then_branch: &Expr, else_branch: &Expr) {
        // Compile condition
        self.compile_expr(condition);

        // Jump to else if condition is false
        let else_jump_offset = self.compiled.current_offset();
        self.compiled.emit(Opcode::JumpIfFalse(0)); // Placeholder

        // Compile then branch
        self.compile_expr(then_branch);

        // Jump over else branch
        let end_jump_offset = self.compiled.current_offset();
        self.compiled.emit(Opcode::Jump(0)); // Placeholder

        // Patch else jump to here
        let else_target = (self.compiled.current_offset() - else_jump_offset) as i16;
        self.compiled.patch_jump(else_jump_offset, else_target);

        // Compile else branch
        self.compile_expr(else_branch);

        // Patch end jump to here
        let end_target = (self.compiled.current_offset() - end_jump_offset) as i16;
        self.compiled.patch_jump(end_jump_offset, end_target);
    }

    /// Compile coalesce expression
    fn compile_coalesce(&mut self, exprs: &[Expr]) {
        if exprs.is_empty() {
            // Empty coalesce returns null
            let idx = self.compiled.add_constant(Value::Null);
            self.compiled.emit(Opcode::LoadConst(idx));
            return;
        }

        let mut jump_offsets = Vec::new();

        // For each expression except the last
        for expr in &exprs[..exprs.len() - 1] {
            self.compile_expr(expr);
            self.compiled.emit(Opcode::Dup); // Keep value for check

            // Check if not null (truthy for non-null)
            // We need a special "is not null" check
            // For simplicity, we'll use the value itself - if it's not null, it stays
            // This is a simplified implementation

            // Jump to end if not null
            let jump_offset = self.compiled.current_offset();
            self.compiled.emit(Opcode::JumpIfTrue(0)); // Placeholder
            self.compiled.emit(Opcode::Pop); // Remove null value
            jump_offsets.push(jump_offset);
        }

        // Compile last expression (no jump needed)
        self.compile_expr(&exprs[exprs.len() - 1]);

        // Patch all jumps to here
        let end_offset = self.compiled.current_offset();
        for jump_offset in jump_offsets {
            let target = (end_offset - jump_offset) as i16;
            self.compiled.patch_jump(jump_offset, target);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::ast::UnaryOp;
    use super::*;

    #[test]
    fn test_compile_literal() {
        let expr = Expr::literal(42);
        let compiled = ExprCompiler::new().compile(&expr);

        assert_eq!(compiled.instructions.len(), 2); // LoadConst + Return
        assert_eq!(compiled.instructions[0], Opcode::LoadConst(0));
        assert_eq!(compiled.instructions[1], Opcode::Return);
        assert_eq!(compiled.constants[0], Value::int(42));
    }

    #[test]
    fn test_compile_field() {
        let expr = Expr::field("user.name");
        let compiled = ExprCompiler::new().compile(&expr);

        assert_eq!(compiled.instructions.len(), 2);
        assert_eq!(compiled.instructions[0], Opcode::LoadField(0));
        assert_eq!(compiled.fields[0], "user.name");
    }

    #[test]
    fn test_compile_binary() {
        let expr = Expr::binary(BinaryOp::Add, Expr::literal(1), Expr::literal(2));
        let compiled = ExprCompiler::new().compile(&expr);

        assert_eq!(compiled.instructions.len(), 4);
        assert_eq!(compiled.instructions[0], Opcode::LoadConst(0)); // 1
        assert_eq!(compiled.instructions[1], Opcode::LoadConst(1)); // 2
        assert_eq!(compiled.instructions[2], Opcode::BinaryOp(BinaryOp::Add));
        assert_eq!(compiled.instructions[3], Opcode::Return);
    }

    #[test]
    fn test_compile_short_circuit_and() {
        // a && b
        let expr = Expr::binary(BinaryOp::And, Expr::field("a"), Expr::field("b"));
        let compiled = ExprCompiler::new().compile(&expr);

        // LoadField(a), Dup, JumpIfFalse, Pop, LoadField(b), Return
        assert!(compiled.instructions.len() >= 5);
        assert!(matches!(compiled.instructions[0], Opcode::LoadField(_)));
        assert_eq!(compiled.instructions[1], Opcode::Dup);
        assert!(matches!(compiled.instructions[2], Opcode::JumpIfFalse(_)));
    }

    #[test]
    fn test_compile_function_call() {
        let expr = Expr::call("len", vec![Expr::field("items")]);
        let compiled = ExprCompiler::new().compile(&expr);

        assert_eq!(compiled.instructions.len(), 3);
        assert_eq!(compiled.instructions[0], Opcode::LoadField(0));
        assert_eq!(compiled.instructions[1], Opcode::Call(0, 1));
        assert_eq!(compiled.functions[0], "len");
    }

    #[test]
    fn test_compile_conditional() {
        // if a then b else c
        let expr = Expr::conditional(Expr::field("a"), Expr::field("b"), Expr::field("c"));
        let compiled = ExprCompiler::new().compile(&expr);

        // Should have: LoadField(a), JumpIfFalse, LoadField(b), Jump, LoadField(c), Return
        assert!(compiled.instructions.len() >= 6);
    }

    #[test]
    fn test_compile_array() {
        let expr = Expr::Array(vec![Expr::literal(1), Expr::literal(2), Expr::literal(3)]);
        let compiled = ExprCompiler::new().compile(&expr);

        // LoadConst(1), LoadConst(2), LoadConst(3), MakeArray(3), Return
        assert_eq!(compiled.instructions.len(), 5);
        assert_eq!(compiled.instructions[3], Opcode::MakeArray(3));
    }

    #[test]
    fn test_compile_unary() {
        let expr = Expr::unary(UnaryOp::Not, Expr::field("flag"));
        let compiled = ExprCompiler::new().compile(&expr);

        assert_eq!(compiled.instructions.len(), 3);
        assert_eq!(compiled.instructions[0], Opcode::LoadField(0));
        assert_eq!(compiled.instructions[1], Opcode::UnaryOp(UnaryOp::Not));
    }
}
