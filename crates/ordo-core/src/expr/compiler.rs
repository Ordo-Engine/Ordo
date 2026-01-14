//! Optimizing compiler for VM v2
//!
//! This compiler generates optimized bytecode for the v2 VM, including:
//! - Superinstruction generation for common patterns
//! - Register allocation
//! - Peephole optimization

use super::ast::{BinaryOp, Expr, UnaryOp};
use super::vm::{CompiledExpr, Instruction, Opcode};
use crate::context::Value;

/// Compiler for VM v2 bytecode
pub struct ExprCompiler {
    /// The compiled expression being built
    compiled: CompiledExpr,
    /// Next available register
    next_reg: u8,
}

impl Default for ExprCompiler {
    fn default() -> Self {
        Self::new()
    }
}

impl ExprCompiler {
    pub fn new() -> Self {
        Self {
            compiled: CompiledExpr::new(),
            next_reg: 0,
        }
    }

    /// Compile an expression to v2 bytecode
    pub fn compile(mut self, expr: &Expr) -> CompiledExpr {
        let result_reg = self.compile_expr(expr);
        self.emit(Instruction::new(Opcode::Return, result_reg, 0, 0));
        self.compiled.register_count = self.next_reg;

        // Apply peephole optimizations
        self.peephole_optimize();

        self.compiled
    }

    /// Allocate a new register
    #[inline]
    fn alloc_reg(&mut self) -> u8 {
        let reg = self.next_reg;
        self.next_reg += 1;
        reg
    }

    /// Emit an instruction
    #[inline]
    fn emit(&mut self, inst: Instruction) {
        self.compiled.instructions.push(inst);
    }

    /// Get current instruction offset
    #[inline]
    fn current_offset(&self) -> usize {
        self.compiled.instructions.len()
    }

    /// Patch a jump instruction
    fn patch_jump(&mut self, offset: usize, target: i16) {
        let inst = &mut self.compiled.instructions[offset];
        inst.b = ((target >> 8) & 0xFF) as u8;
        inst.c = (target & 0xFF) as u8;
    }

    /// Add a constant to the pool
    fn add_constant(&mut self, value: Value) -> u8 {
        // Check if constant already exists
        if let Some(idx) = self.compiled.constants.iter().position(|v| v == &value) {
            return idx as u8;
        }
        let idx = self.compiled.constants.len();
        self.compiled.constants.push(value);
        idx as u8
    }

    /// Add a field to the pool
    fn add_field(&mut self, name: &str) -> u8 {
        if let Some(idx) = self.compiled.fields.iter().position(|f| f == name) {
            return idx as u8;
        }
        let idx = self.compiled.fields.len();
        self.compiled.fields.push(name.to_string());
        idx as u8
    }

    /// Add a function to the pool
    fn add_function(&mut self, name: &str) -> u8 {
        if let Some(idx) = self.compiled.functions.iter().position(|f| f == name) {
            return idx as u8;
        }
        let idx = self.compiled.functions.len();
        self.compiled.functions.push(name.to_string());
        idx as u8
    }

    /// Compile an expression, returning the register containing the result
    fn compile_expr(&mut self, expr: &Expr) -> u8 {
        match expr {
            Expr::Literal(value) => {
                let reg = self.alloc_reg();
                let const_idx = self.add_constant(value.clone());
                self.emit(Instruction::new(Opcode::LoadConst, reg, const_idx, 0));
                reg
            }

            Expr::Field(path) => {
                let reg = self.alloc_reg();
                let field_idx = self.add_field(path);
                self.emit(Instruction::new(Opcode::LoadField, reg, field_idx, 0));
                reg
            }

            Expr::Binary { op, left, right } => {
                // Try to generate superinstruction first
                if let Some(reg) = self.try_superinstruction(*op, left, right) {
                    return reg;
                }

                // Handle short-circuit operators specially
                match op {
                    BinaryOp::And => self.compile_and(left, right),
                    BinaryOp::Or => self.compile_or(left, right),
                    _ => {
                        let left_reg = self.compile_expr(left);
                        let right_reg = self.compile_expr(right);
                        let result_reg = self.alloc_reg();
                        let opcode = binary_op_to_opcode(*op);
                        self.emit(Instruction::new(opcode, result_reg, left_reg, right_reg));
                        result_reg
                    }
                }
            }

            Expr::Unary { op, operand } => {
                let operand_reg = self.compile_expr(operand);
                let result_reg = self.alloc_reg();
                let opcode = match op {
                    UnaryOp::Not => Opcode::Not,
                    UnaryOp::Neg => Opcode::Neg,
                };
                self.emit(Instruction::new(opcode, result_reg, operand_reg, 0));
                result_reg
            }

            Expr::Call { name, args } => {
                // Allocate result register first, then argument registers
                let result_reg = self.alloc_reg();

                // Compile arguments into consecutive registers after result
                let arg_start = self.next_reg;
                for arg in args {
                    let arg_reg = self.compile_expr(arg);
                    // Move to consecutive position if needed
                    if arg_reg != self.next_reg - 1 {
                        let target = self.alloc_reg();
                        self.emit(Instruction::new(Opcode::Move, target, arg_reg, 0));
                    }
                }

                let func_idx = self.add_function(name);
                // For Call: a=result, b=func_idx, c=arg_count
                // Arguments are expected in registers [result_reg+1, result_reg+1+arg_count)
                self.emit(Instruction::new(
                    Opcode::Call,
                    result_reg,
                    func_idx,
                    args.len() as u8,
                ));

                // Reset next_reg to reuse argument registers
                self.next_reg = arg_start;

                result_reg
            }

            Expr::Conditional {
                condition,
                then_branch,
                else_branch,
            } => self.compile_conditional(condition, then_branch, else_branch),

            Expr::Array(elements) => {
                // For now, compile each element and return the last register
                // A proper implementation would create an array value
                if elements.is_empty() {
                    let reg = self.alloc_reg();
                    let const_idx = self.add_constant(Value::array(vec![]));
                    self.emit(Instruction::new(Opcode::LoadConst, reg, const_idx, 0));
                    return reg;
                }

                // Compile as constant if all elements are literals
                if elements.iter().all(|e| matches!(e, Expr::Literal(_))) {
                    let values: Vec<Value> = elements
                        .iter()
                        .filter_map(|e| {
                            if let Expr::Literal(v) = e {
                                Some(v.clone())
                            } else {
                                None
                            }
                        })
                        .collect();
                    let reg = self.alloc_reg();
                    let const_idx = self.add_constant(Value::array(values));
                    self.emit(Instruction::new(Opcode::LoadConst, reg, const_idx, 0));
                    return reg;
                }

                // Fall back to loading the first element for now
                self.compile_expr(&elements[0])
            }

            Expr::Object(_pairs) => {
                // Simplified: return empty object
                let reg = self.alloc_reg();
                let const_idx = self.add_constant(Value::object(std::collections::HashMap::new()));
                self.emit(Instruction::new(Opcode::LoadConst, reg, const_idx, 0));
                reg
            }

            Expr::Exists(path) => {
                let reg = self.alloc_reg();
                let field_idx = self.add_field(path);
                self.emit(Instruction::new(Opcode::Exists, reg, field_idx, 0));
                reg
            }

            Expr::Coalesce(exprs) => self.compile_coalesce(exprs),
        }
    }

    /// Try to generate a superinstruction for common patterns
    fn try_superinstruction(&mut self, op: BinaryOp, left: &Expr, right: &Expr) -> Option<u8> {
        // Pattern: field > constant
        if let (Expr::Field(field), Expr::Literal(value)) = (left, right) {
            let superop = match op {
                BinaryOp::Gt => Opcode::FieldGtConst,
                BinaryOp::Lt => Opcode::FieldLtConst,
                BinaryOp::Eq => Opcode::FieldEqConst,
                BinaryOp::Ne => Opcode::FieldNeConst,
                BinaryOp::Ge => Opcode::FieldGeConst,
                BinaryOp::Le => Opcode::FieldLeConst,
                _ => return None,
            };

            let reg = self.alloc_reg();
            let field_idx = self.add_field(field);
            let const_idx = self.add_constant(value.clone());
            self.emit(Instruction::new(superop, reg, field_idx, const_idx));
            return Some(reg);
        }

        // Pattern: constant < field (reverse comparison)
        if let (Expr::Literal(value), Expr::Field(field)) = (left, right) {
            let superop = match op {
                BinaryOp::Lt => Opcode::FieldGtConst, // const < field => field > const
                BinaryOp::Gt => Opcode::FieldLtConst, // const > field => field < const
                BinaryOp::Le => Opcode::FieldGeConst, // const <= field => field >= const
                BinaryOp::Ge => Opcode::FieldLeConst, // const >= field => field <= const
                BinaryOp::Eq => Opcode::FieldEqConst,
                BinaryOp::Ne => Opcode::FieldNeConst,
                _ => return None,
            };

            let reg = self.alloc_reg();
            let field_idx = self.add_field(field);
            let const_idx = self.add_constant(value.clone());
            self.emit(Instruction::new(superop, reg, field_idx, const_idx));
            return Some(reg);
        }

        None
    }

    /// Compile short-circuit AND
    fn compile_and(&mut self, left: &Expr, right: &Expr) -> u8 {
        let left_reg = self.compile_expr(left);
        let result_reg = self.alloc_reg();

        // Copy left to result (in case we short-circuit)
        self.emit(Instruction::new(Opcode::Move, result_reg, left_reg, 0));

        // Jump if false
        let jump_offset = self.current_offset();
        self.emit(Instruction::with_offset(Opcode::JumpIfFalse, result_reg, 0));

        // Evaluate right side
        let right_reg = self.compile_expr(right);
        self.emit(Instruction::new(Opcode::Move, result_reg, right_reg, 0));

        // Patch jump
        let target = (self.current_offset() - jump_offset) as i16;
        self.patch_jump(jump_offset, target);

        result_reg
    }

    /// Compile short-circuit OR
    fn compile_or(&mut self, left: &Expr, right: &Expr) -> u8 {
        let left_reg = self.compile_expr(left);
        let result_reg = self.alloc_reg();

        // Copy left to result
        self.emit(Instruction::new(Opcode::Move, result_reg, left_reg, 0));

        // Jump if true
        let jump_offset = self.current_offset();
        self.emit(Instruction::with_offset(Opcode::JumpIfTrue, result_reg, 0));

        // Evaluate right side
        let right_reg = self.compile_expr(right);
        self.emit(Instruction::new(Opcode::Move, result_reg, right_reg, 0));

        // Patch jump
        let target = (self.current_offset() - jump_offset) as i16;
        self.patch_jump(jump_offset, target);

        result_reg
    }

    /// Compile conditional expression
    fn compile_conditional(
        &mut self,
        condition: &Expr,
        then_branch: &Expr,
        else_branch: &Expr,
    ) -> u8 {
        let cond_reg = self.compile_expr(condition);
        let result_reg = self.alloc_reg();

        // Jump to else if false
        let else_jump = self.current_offset();
        self.emit(Instruction::with_offset(Opcode::JumpIfFalse, cond_reg, 0));

        // Then branch
        let then_reg = self.compile_expr(then_branch);
        self.emit(Instruction::new(Opcode::Move, result_reg, then_reg, 0));

        // Jump over else
        let end_jump = self.current_offset();
        self.emit(Instruction::with_offset(Opcode::Jump, 0, 0));

        // Patch else jump
        let else_target = (self.current_offset() - else_jump) as i16;
        self.patch_jump(else_jump, else_target);

        // Else branch
        let else_reg = self.compile_expr(else_branch);
        self.emit(Instruction::new(Opcode::Move, result_reg, else_reg, 0));

        // Patch end jump
        let end_target = (self.current_offset() - end_jump) as i16;
        self.patch_jump(end_jump, end_target);

        result_reg
    }

    /// Compile coalesce expression
    fn compile_coalesce(&mut self, exprs: &[Expr]) -> u8 {
        if exprs.is_empty() {
            let reg = self.alloc_reg();
            let const_idx = self.add_constant(Value::Null);
            self.emit(Instruction::new(Opcode::LoadConst, reg, const_idx, 0));
            return reg;
        }

        let result_reg = self.alloc_reg();
        let mut jump_offsets = Vec::new();

        for expr in &exprs[..exprs.len() - 1] {
            let expr_reg = self.compile_expr(expr);
            self.emit(Instruction::new(Opcode::Move, result_reg, expr_reg, 0));

            // Jump to end if truthy (not null)
            let jump_offset = self.current_offset();
            self.emit(Instruction::with_offset(Opcode::JumpIfTrue, result_reg, 0));
            jump_offsets.push(jump_offset);
        }

        // Last expression
        let last_reg = self.compile_expr(&exprs[exprs.len() - 1]);
        self.emit(Instruction::new(Opcode::Move, result_reg, last_reg, 0));

        // Patch all jumps to here
        let end_offset = self.current_offset();
        for jump_offset in jump_offsets {
            let target = (end_offset - jump_offset) as i16;
            self.patch_jump(jump_offset, target);
        }

        result_reg
    }

    /// Apply peephole optimizations
    fn peephole_optimize(&mut self) {
        // Remove redundant moves (Move r, r)
        self.compiled
            .instructions
            .retain(|inst| !(inst.op == Opcode::Move && inst.a == inst.b));

        // TODO: More peephole optimizations
        // - Combine LoadConst + BinaryOp into superinstruction
        // - Remove dead code after unconditional jumps
        // - Strength reduction (x * 2 -> x + x)
    }
}

/// Convert BinaryOp to Opcode
fn binary_op_to_opcode(op: BinaryOp) -> Opcode {
    match op {
        BinaryOp::Add => Opcode::Add,
        BinaryOp::Sub => Opcode::Sub,
        BinaryOp::Mul => Opcode::Mul,
        BinaryOp::Div => Opcode::Div,
        BinaryOp::Mod => Opcode::Mod,
        BinaryOp::Eq => Opcode::Eq,
        BinaryOp::Ne => Opcode::Ne,
        BinaryOp::Lt => Opcode::Lt,
        BinaryOp::Le => Opcode::Le,
        BinaryOp::Gt => Opcode::Gt,
        BinaryOp::Ge => Opcode::Ge,
        BinaryOp::And => Opcode::And,
        BinaryOp::Or => Opcode::Or,
        BinaryOp::In => Opcode::In,
        BinaryOp::NotIn => Opcode::NotIn,
        BinaryOp::Contains => Opcode::Contains,
    }
}

#[cfg(test)]
mod tests {
    use super::super::vm::BytecodeVM;
    use super::*;

    fn make_ctx(json: &str) -> crate::context::Context {
        crate::context::Context::from_json(json).unwrap()
    }

    fn compile_and_run(expr: &Expr, ctx: &crate::context::Context) -> crate::error::Result<Value> {
        let compiled = ExprCompiler::new().compile(expr);
        let vm = BytecodeVM::new();
        vm.execute(&compiled, ctx)
    }

    #[test]
    fn test_compile_v2_literal() {
        let expr = Expr::literal(42);
        let ctx = crate::context::Context::default();
        let result = compile_and_run(&expr, &ctx).unwrap();
        assert_eq!(result, Value::int(42));
    }

    #[test]
    fn test_compile_v2_field() {
        let expr = Expr::field("age");
        let ctx = make_ctx(r#"{"age": 25}"#);
        let result = compile_and_run(&expr, &ctx).unwrap();
        assert_eq!(result, Value::int(25));
    }

    #[test]
    fn test_compile_v2_superinstruction() {
        // age > 18 should use FieldGtConst superinstruction
        let expr = Expr::binary(BinaryOp::Gt, Expr::field("age"), Expr::literal(18));
        let ctx = make_ctx(r#"{"age": 25}"#);

        let compiled = ExprCompiler::new().compile(&expr);

        // Should have 2 instructions: FieldGtConst + Return
        assert_eq!(compiled.instructions.len(), 2);
        assert_eq!(compiled.instructions[0].op, Opcode::FieldGtConst);

        let vm = BytecodeVM::new();
        let result = vm.execute(&compiled, &ctx).unwrap();
        assert_eq!(result, Value::bool(true));
    }

    #[test]
    fn test_compile_v2_arithmetic() {
        let ctx = make_ctx(r#"{"a": 10, "b": 3}"#);

        // a + b
        let expr = Expr::binary(BinaryOp::Add, Expr::field("a"), Expr::field("b"));
        let result = compile_and_run(&expr, &ctx).unwrap();
        assert_eq!(result, Value::int(13));

        // a * b
        let expr = Expr::binary(BinaryOp::Mul, Expr::field("a"), Expr::field("b"));
        let result = compile_and_run(&expr, &ctx).unwrap();
        assert_eq!(result, Value::int(30));
    }

    #[test]
    fn test_compile_v2_logical_and() {
        let ctx = make_ctx(r#"{"a": true, "b": false}"#);

        // a && b = false
        let expr = Expr::binary(BinaryOp::And, Expr::field("a"), Expr::field("b"));
        let result = compile_and_run(&expr, &ctx).unwrap();
        assert_eq!(result, Value::bool(false));
    }

    #[test]
    fn test_compile_v2_logical_or() {
        let ctx = make_ctx(r#"{"a": false, "b": true}"#);

        // a || b = true
        let expr = Expr::binary(BinaryOp::Or, Expr::field("a"), Expr::field("b"));
        let result = compile_and_run(&expr, &ctx).unwrap();
        assert_eq!(result, Value::bool(true));
    }

    #[test]
    fn test_compile_v2_conditional() {
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
    fn test_compile_v2_complex() {
        let ctx = make_ctx(r#"{"age": 25, "status": "active"}"#);

        // age > 18 && status == "active"
        let expr = Expr::binary(
            BinaryOp::And,
            Expr::binary(BinaryOp::Gt, Expr::field("age"), Expr::literal(18)),
            Expr::binary(BinaryOp::Eq, Expr::field("status"), Expr::literal("active")),
        );

        let result = compile_and_run(&expr, &ctx).unwrap();
        assert_eq!(result, Value::bool(true));
    }
}
