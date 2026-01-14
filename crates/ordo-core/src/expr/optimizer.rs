//! Expression optimizer
//!
//! Performs compile-time optimizations on expression AST:
//! - Constant folding: evaluate constant sub-expressions at compile time
//! - Dead code elimination: simplify conditional expressions with constant conditions
//! - Algebraic simplification: simplify expressions like `x * 1`, `x + 0`, etc.

use super::ast::{BinaryOp, Expr, UnaryOp};
use crate::context::Value;

/// Expression optimizer that performs compile-time optimizations
#[derive(Debug, Default)]
pub struct ExprOptimizer {
    /// Statistics about optimizations performed
    stats: OptimizationStats,
}

/// Statistics about optimizations performed
#[derive(Debug, Default, Clone)]
pub struct OptimizationStats {
    /// Number of constant folding operations
    pub constant_folds: usize,
    /// Number of dead code eliminations
    pub dead_code_eliminations: usize,
    /// Number of algebraic simplifications
    pub algebraic_simplifications: usize,
}

impl ExprOptimizer {
    /// Create a new optimizer
    pub fn new() -> Self {
        Self::default()
    }

    /// Get optimization statistics
    pub fn stats(&self) -> &OptimizationStats {
        &self.stats
    }

    /// Reset statistics
    pub fn reset_stats(&mut self) {
        self.stats = OptimizationStats::default();
    }

    /// Optimize an expression
    pub fn optimize(&mut self, expr: Expr) -> Expr {
        self.optimize_recursive(expr)
    }

    /// Recursively optimize an expression
    fn optimize_recursive(&mut self, expr: Expr) -> Expr {
        match expr {
            // Literals and fields cannot be optimized further
            Expr::Literal(_) | Expr::Field(_) | Expr::Exists(_) => expr,

            // Optimize binary operations
            Expr::Binary { op, left, right } => {
                let left = self.optimize_recursive(*left);
                let right = self.optimize_recursive(*right);
                self.optimize_binary(op, left, right)
            }

            // Optimize unary operations
            Expr::Unary { op, operand } => {
                let operand = self.optimize_recursive(*operand);
                self.optimize_unary(op, operand)
            }

            // Optimize function calls (constant propagation for pure functions)
            Expr::Call { name, args } => {
                let args: Vec<Expr> = args
                    .into_iter()
                    .map(|a| self.optimize_recursive(a))
                    .collect();
                self.optimize_call(name, args)
            }

            // Optimize conditional expressions
            Expr::Conditional {
                condition,
                then_branch,
                else_branch,
            } => {
                let condition = self.optimize_recursive(*condition);
                let then_branch = self.optimize_recursive(*then_branch);
                let else_branch = self.optimize_recursive(*else_branch);
                self.optimize_conditional(condition, then_branch, else_branch)
            }

            // Optimize arrays
            Expr::Array(elements) => {
                let elements: Vec<Expr> = elements
                    .into_iter()
                    .map(|e| self.optimize_recursive(e))
                    .collect();

                // If all elements are literals, create a literal array
                if elements.iter().all(|e| matches!(e, Expr::Literal(_))) {
                    let values: Vec<Value> = elements
                        .into_iter()
                        .filter_map(|e| {
                            if let Expr::Literal(v) = e {
                                Some(v)
                            } else {
                                None
                            }
                        })
                        .collect();
                    self.stats.constant_folds += 1;
                    Expr::Literal(Value::array(values))
                } else {
                    Expr::Array(elements)
                }
            }

            // Optimize objects
            Expr::Object(pairs) => {
                let pairs: Vec<(String, Expr)> = pairs
                    .into_iter()
                    .map(|(k, v)| (k, self.optimize_recursive(v)))
                    .collect();
                Expr::Object(pairs)
            }

            // Optimize coalesce
            Expr::Coalesce(exprs) => {
                let exprs: Vec<Expr> = exprs
                    .into_iter()
                    .map(|e| self.optimize_recursive(e))
                    .collect();
                self.optimize_coalesce(exprs)
            }
        }
    }

    /// Optimize binary operations
    fn optimize_binary(&mut self, op: BinaryOp, left: Expr, right: Expr) -> Expr {
        // Try constant folding first
        if let (Expr::Literal(l), Expr::Literal(r)) = (&left, &right) {
            if let Some(result) = self.fold_binary_constants(op, l, r) {
                self.stats.constant_folds += 1;
                return Expr::Literal(result);
            }
        }

        // Algebraic simplifications
        if let Some(simplified) = self.simplify_binary(op, &left, &right) {
            self.stats.algebraic_simplifications += 1;
            return simplified;
        }

        Expr::Binary {
            op,
            left: Box::new(left),
            right: Box::new(right),
        }
    }

    /// Fold binary operation on constant values
    fn fold_binary_constants(&self, op: BinaryOp, left: &Value, right: &Value) -> Option<Value> {
        match op {
            // Arithmetic
            BinaryOp::Add => self.fold_add(left, right),
            BinaryOp::Sub => self.fold_sub(left, right),
            BinaryOp::Mul => self.fold_mul(left, right),
            BinaryOp::Div => self.fold_div(left, right),
            BinaryOp::Mod => self.fold_mod(left, right),

            // Comparison
            BinaryOp::Eq => Some(Value::bool(left == right)),
            BinaryOp::Ne => Some(Value::bool(left != right)),
            BinaryOp::Lt => left
                .compare(right)
                .map(|o| Value::bool(o == std::cmp::Ordering::Less)),
            BinaryOp::Le => left
                .compare(right)
                .map(|o| Value::bool(o != std::cmp::Ordering::Greater)),
            BinaryOp::Gt => left
                .compare(right)
                .map(|o| Value::bool(o == std::cmp::Ordering::Greater)),
            BinaryOp::Ge => left
                .compare(right)
                .map(|o| Value::bool(o != std::cmp::Ordering::Less)),

            // Logical
            BinaryOp::And => {
                let l = left.is_truthy();
                let r = right.is_truthy();
                Some(Value::bool(l && r))
            }
            BinaryOp::Or => {
                let l = left.is_truthy();
                let r = right.is_truthy();
                Some(Value::bool(l || r))
            }

            // Set operations - can be folded for constant arrays
            BinaryOp::In => {
                if let Value::Array(arr) = right {
                    Some(Value::bool(arr.contains(left)))
                } else if let (Value::String(l), Value::String(r)) = (left, right) {
                    Some(Value::bool(r.contains(l.as_ref())))
                } else {
                    None
                }
            }
            BinaryOp::NotIn => {
                if let Value::Array(arr) = right {
                    Some(Value::bool(!arr.contains(left)))
                } else if let (Value::String(l), Value::String(r)) = (left, right) {
                    Some(Value::bool(!r.contains(l.as_ref())))
                } else {
                    None
                }
            }
            BinaryOp::Contains => {
                if let Value::Array(arr) = left {
                    Some(Value::bool(arr.contains(right)))
                } else if let (Value::String(l), Value::String(r)) = (left, right) {
                    Some(Value::bool(l.contains(r.as_ref())))
                } else {
                    None
                }
            }
        }
    }

    /// Fold addition
    fn fold_add(&self, left: &Value, right: &Value) -> Option<Value> {
        match (left, right) {
            (Value::Int(a), Value::Int(b)) => a.checked_add(*b).map(Value::int),
            (Value::Float(a), Value::Float(b)) => Some(Value::float(a + b)),
            (Value::Int(a), Value::Float(b)) => Some(Value::float(*a as f64 + b)),
            (Value::Float(a), Value::Int(b)) => Some(Value::float(a + *b as f64)),
            (Value::String(a), Value::String(b)) => Some(Value::string(format!("{}{}", a, b))),
            _ => None,
        }
    }

    /// Fold subtraction
    fn fold_sub(&self, left: &Value, right: &Value) -> Option<Value> {
        match (left, right) {
            (Value::Int(a), Value::Int(b)) => a.checked_sub(*b).map(Value::int),
            (Value::Float(a), Value::Float(b)) => Some(Value::float(a - b)),
            (Value::Int(a), Value::Float(b)) => Some(Value::float(*a as f64 - b)),
            (Value::Float(a), Value::Int(b)) => Some(Value::float(a - *b as f64)),
            _ => None,
        }
    }

    /// Fold multiplication
    fn fold_mul(&self, left: &Value, right: &Value) -> Option<Value> {
        match (left, right) {
            (Value::Int(a), Value::Int(b)) => a.checked_mul(*b).map(Value::int),
            (Value::Float(a), Value::Float(b)) => Some(Value::float(a * b)),
            (Value::Int(a), Value::Float(b)) => Some(Value::float(*a as f64 * b)),
            (Value::Float(a), Value::Int(b)) => Some(Value::float(a * *b as f64)),
            _ => None,
        }
    }

    /// Fold division
    fn fold_div(&self, left: &Value, right: &Value) -> Option<Value> {
        match (left, right) {
            (Value::Int(a), Value::Int(b)) if *b != 0 => Some(Value::int(a / b)),
            (Value::Float(a), Value::Float(b)) if *b != 0.0 => Some(Value::float(a / b)),
            (Value::Int(a), Value::Float(b)) if *b != 0.0 => Some(Value::float(*a as f64 / b)),
            (Value::Float(a), Value::Int(b)) if *b != 0 => Some(Value::float(a / *b as f64)),
            _ => None,
        }
    }

    /// Fold modulo
    fn fold_mod(&self, left: &Value, right: &Value) -> Option<Value> {
        match (left, right) {
            (Value::Int(a), Value::Int(b)) if *b != 0 => Some(Value::int(a % b)),
            _ => None,
        }
    }

    /// Algebraic simplifications for binary operations
    fn simplify_binary(&self, op: BinaryOp, left: &Expr, right: &Expr) -> Option<Expr> {
        match op {
            // x + 0 = x, 0 + x = x
            BinaryOp::Add => {
                if is_zero(right) {
                    return Some(left.clone());
                }
                if is_zero(left) {
                    return Some(right.clone());
                }
                None
            }

            // x - 0 = x
            BinaryOp::Sub => {
                if is_zero(right) {
                    return Some(left.clone());
                }
                None
            }

            // x * 1 = x, 1 * x = x, x * 0 = 0, 0 * x = 0
            BinaryOp::Mul => {
                if is_one(right) {
                    return Some(left.clone());
                }
                if is_one(left) {
                    return Some(right.clone());
                }
                if is_zero(right) || is_zero(left) {
                    return Some(Expr::Literal(Value::int(0)));
                }
                None
            }

            // x / 1 = x
            BinaryOp::Div => {
                if is_one(right) {
                    return Some(left.clone());
                }
                None
            }

            // true && x = x, false && x = false
            BinaryOp::And => {
                if let Expr::Literal(Value::Bool(true)) = left {
                    return Some(right.clone());
                }
                if let Expr::Literal(Value::Bool(false)) = left {
                    return Some(Expr::Literal(Value::bool(false)));
                }
                if let Expr::Literal(Value::Bool(true)) = right {
                    return Some(left.clone());
                }
                if let Expr::Literal(Value::Bool(false)) = right {
                    return Some(Expr::Literal(Value::bool(false)));
                }
                None
            }

            // true || x = true, false || x = x
            BinaryOp::Or => {
                if let Expr::Literal(Value::Bool(true)) = left {
                    return Some(Expr::Literal(Value::bool(true)));
                }
                if let Expr::Literal(Value::Bool(false)) = left {
                    return Some(right.clone());
                }
                if let Expr::Literal(Value::Bool(true)) = right {
                    return Some(Expr::Literal(Value::bool(true)));
                }
                if let Expr::Literal(Value::Bool(false)) = right {
                    return Some(left.clone());
                }
                None
            }

            _ => None,
        }
    }

    /// Optimize unary operations
    fn optimize_unary(&mut self, op: UnaryOp, operand: Expr) -> Expr {
        // Constant folding for unary operations
        if let Expr::Literal(v) = &operand {
            if let Some(result) = self.fold_unary_constant(op, v) {
                self.stats.constant_folds += 1;
                return Expr::Literal(result);
            }
        }

        // Double negation elimination: !!x = x
        if op == UnaryOp::Not {
            if let Expr::Unary {
                op: UnaryOp::Not,
                operand: inner,
            } = operand
            {
                self.stats.algebraic_simplifications += 1;
                return *inner;
            }
        }

        // Double arithmetic negation: --x = x
        if op == UnaryOp::Neg {
            if let Expr::Unary {
                op: UnaryOp::Neg,
                operand: inner,
            } = operand
            {
                self.stats.algebraic_simplifications += 1;
                return *inner;
            }
        }

        Expr::Unary {
            op,
            operand: Box::new(operand),
        }
    }

    /// Fold unary operation on constant value
    fn fold_unary_constant(&self, op: UnaryOp, value: &Value) -> Option<Value> {
        match op {
            UnaryOp::Not => Some(Value::bool(!value.is_truthy())),
            UnaryOp::Neg => match value {
                Value::Int(n) => Some(Value::int(-n)),
                Value::Float(n) => Some(Value::float(-n)),
                _ => None,
            },
        }
    }

    /// Optimize function calls (constant propagation for pure functions)
    fn optimize_call(&mut self, name: String, args: Vec<Expr>) -> Expr {
        // Only fold pure functions with all constant arguments
        if args.iter().all(|a| matches!(a, Expr::Literal(_))) {
            let values: Vec<Value> = args
                .iter()
                .filter_map(|a| {
                    if let Expr::Literal(v) = a {
                        Some(v.clone())
                    } else {
                        None
                    }
                })
                .collect();

            if let Some(result) = self.fold_pure_function(&name, &values) {
                self.stats.constant_folds += 1;
                return Expr::Literal(result);
            }
        }

        Expr::Call { name, args }
    }

    /// Fold pure function with constant arguments
    fn fold_pure_function(&self, name: &str, args: &[Value]) -> Option<Value> {
        match name {
            // Math functions
            "abs" if args.len() == 1 => match &args[0] {
                Value::Int(n) => Some(Value::int(n.abs())),
                Value::Float(n) => Some(Value::float(n.abs())),
                _ => None,
            },
            "min" if args.len() >= 2 => {
                let mut min = args[0].clone();
                for arg in &args[1..] {
                    if let Some(std::cmp::Ordering::Greater) = min.compare(arg) {
                        min = arg.clone();
                    }
                }
                Some(min)
            }
            "max" if args.len() >= 2 => {
                let mut max = args[0].clone();
                for arg in &args[1..] {
                    if let Some(std::cmp::Ordering::Less) = max.compare(arg) {
                        max = arg.clone();
                    }
                }
                Some(max)
            }
            "floor" if args.len() == 1 => {
                if let Value::Float(n) = &args[0] {
                    Some(Value::float(n.floor()))
                } else {
                    None
                }
            }
            "ceil" if args.len() == 1 => {
                if let Value::Float(n) = &args[0] {
                    Some(Value::float(n.ceil()))
                } else {
                    None
                }
            }
            "round" if args.len() == 1 => {
                if let Value::Float(n) = &args[0] {
                    Some(Value::float(n.round()))
                } else {
                    None
                }
            }

            // String functions
            "len" if args.len() == 1 => match &args[0] {
                Value::String(s) => Some(Value::int(s.len() as i64)),
                Value::Array(arr) => Some(Value::int(arr.len() as i64)),
                _ => None,
            },
            "upper" if args.len() == 1 => {
                if let Value::String(s) = &args[0] {
                    Some(Value::string(s.to_uppercase()))
                } else {
                    None
                }
            }
            "lower" if args.len() == 1 => {
                if let Value::String(s) = &args[0] {
                    Some(Value::string(s.to_lowercase()))
                } else {
                    None
                }
            }
            "trim" if args.len() == 1 => {
                if let Value::String(s) = &args[0] {
                    Some(Value::string(s.trim()))
                } else {
                    None
                }
            }

            // Array functions
            "sum" if args.len() == 1 => {
                if let Value::Array(arr) = &args[0] {
                    let mut sum = 0.0;
                    for v in arr.iter() {
                        match v {
                            Value::Int(n) => sum += *n as f64,
                            Value::Float(n) => sum += n,
                            _ => return None,
                        }
                    }
                    Some(Value::float(sum))
                } else {
                    None
                }
            }
            "avg" if args.len() == 1 => {
                if let Value::Array(arr) = &args[0] {
                    if arr.is_empty() {
                        return Some(Value::float(0.0));
                    }
                    let mut sum = 0.0;
                    for v in arr.iter() {
                        match v {
                            Value::Int(n) => sum += *n as f64,
                            Value::Float(n) => sum += n,
                            _ => return None,
                        }
                    }
                    Some(Value::float(sum / arr.len() as f64))
                } else {
                    None
                }
            }
            "count" if args.len() == 1 => {
                if let Value::Array(arr) = &args[0] {
                    Some(Value::int(arr.len() as i64))
                } else {
                    None
                }
            }
            "first" if args.len() == 1 => {
                if let Value::Array(arr) = &args[0] {
                    arr.first().cloned().or(Some(Value::Null))
                } else {
                    None
                }
            }
            "last" if args.len() == 1 => {
                if let Value::Array(arr) = &args[0] {
                    arr.last().cloned().or(Some(Value::Null))
                } else {
                    None
                }
            }

            // Type functions
            "type" if args.len() == 1 => Some(Value::string(args[0].type_name())),
            "is_null" if args.len() == 1 => Some(Value::bool(args[0].is_null())),
            "is_number" if args.len() == 1 => Some(Value::bool(matches!(
                args[0],
                Value::Int(_) | Value::Float(_)
            ))),
            "is_string" if args.len() == 1 => {
                Some(Value::bool(matches!(args[0], Value::String(_))))
            }
            "is_array" if args.len() == 1 => Some(Value::bool(matches!(args[0], Value::Array(_)))),

            // Conversion functions
            "to_int" if args.len() == 1 => match &args[0] {
                Value::Int(n) => Some(Value::int(*n)),
                Value::Float(n) => Some(Value::int(*n as i64)),
                Value::String(s) => s.parse::<i64>().ok().map(Value::int),
                Value::Bool(b) => Some(Value::int(if *b { 1 } else { 0 })),
                _ => None,
            },
            "to_float" if args.len() == 1 => match &args[0] {
                Value::Int(n) => Some(Value::float(*n as f64)),
                Value::Float(n) => Some(Value::float(*n)),
                Value::String(s) => s.parse::<f64>().ok().map(Value::float),
                _ => None,
            },
            "to_string" if args.len() == 1 => Some(Value::string(format!("{}", args[0]))),

            _ => None,
        }
    }

    /// Optimize conditional expressions
    fn optimize_conditional(
        &mut self,
        condition: Expr,
        then_branch: Expr,
        else_branch: Expr,
    ) -> Expr {
        // Dead code elimination: if constant condition
        if let Expr::Literal(v) = &condition {
            self.stats.dead_code_eliminations += 1;
            if v.is_truthy() {
                return then_branch;
            } else {
                return else_branch;
            }
        }

        // If both branches are the same, return one of them
        if then_branch == else_branch {
            self.stats.dead_code_eliminations += 1;
            return then_branch;
        }

        Expr::Conditional {
            condition: Box::new(condition),
            then_branch: Box::new(then_branch),
            else_branch: Box::new(else_branch),
        }
    }

    /// Optimize coalesce expressions
    fn optimize_coalesce(&mut self, exprs: Vec<Expr>) -> Expr {
        let mut optimized = Vec::new();

        for expr in exprs {
            // If we find a non-null literal, stop here
            if let Expr::Literal(v) = &expr {
                if !v.is_null() {
                    self.stats.dead_code_eliminations += 1;
                    optimized.push(expr);
                    break;
                }
                // Skip null literals
                continue;
            }
            optimized.push(expr);
        }

        // If only one expression left, return it directly
        if optimized.len() == 1 {
            return optimized.into_iter().next().unwrap();
        }

        // If no expressions left, return null
        if optimized.is_empty() {
            return Expr::Literal(Value::Null);
        }

        Expr::Coalesce(optimized)
    }
}

/// Check if an expression is the literal value 0
fn is_zero(expr: &Expr) -> bool {
    match expr {
        Expr::Literal(Value::Int(0)) => true,
        Expr::Literal(Value::Float(f)) => *f == 0.0,
        _ => false,
    }
}

/// Check if an expression is the literal value 1
fn is_one(expr: &Expr) -> bool {
    match expr {
        Expr::Literal(Value::Int(1)) => true,
        Expr::Literal(Value::Float(f)) => *f == 1.0,
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_constant_folding_arithmetic() {
        let mut opt = ExprOptimizer::new();

        // 1 + 2 = 3
        let expr = Expr::binary(BinaryOp::Add, Expr::literal(1), Expr::literal(2));
        let optimized = opt.optimize(expr);
        assert_eq!(optimized, Expr::Literal(Value::int(3)));
        assert_eq!(opt.stats().constant_folds, 1);

        opt.reset_stats();

        // 10 * 5 = 50
        let expr = Expr::binary(BinaryOp::Mul, Expr::literal(10), Expr::literal(5));
        let optimized = opt.optimize(expr);
        assert_eq!(optimized, Expr::Literal(Value::int(50)));

        opt.reset_stats();

        // (1 - 0.2) = 0.8
        let expr = Expr::binary(BinaryOp::Sub, Expr::literal(1.0f64), Expr::literal(0.2f64));
        let optimized = opt.optimize(expr);
        if let Expr::Literal(Value::Float(f)) = optimized {
            assert!((f - 0.8).abs() < 0.0001);
        } else {
            panic!("Expected float literal");
        }
    }

    #[test]
    fn test_constant_folding_comparison() {
        let mut opt = ExprOptimizer::new();

        // 5 > 3 = true
        let expr = Expr::binary(BinaryOp::Gt, Expr::literal(5), Expr::literal(3));
        let optimized = opt.optimize(expr);
        assert_eq!(optimized, Expr::Literal(Value::bool(true)));

        // 2 == 2 = true
        let expr = Expr::binary(BinaryOp::Eq, Expr::literal(2), Expr::literal(2));
        let optimized = opt.optimize(expr);
        assert_eq!(optimized, Expr::Literal(Value::bool(true)));
    }

    #[test]
    fn test_constant_folding_logical() {
        let mut opt = ExprOptimizer::new();

        // true && false = false
        let expr = Expr::binary(BinaryOp::And, Expr::literal(true), Expr::literal(false));
        let optimized = opt.optimize(expr);
        assert_eq!(optimized, Expr::Literal(Value::bool(false)));

        // true || false = true
        let expr = Expr::binary(BinaryOp::Or, Expr::literal(true), Expr::literal(false));
        let optimized = opt.optimize(expr);
        assert_eq!(optimized, Expr::Literal(Value::bool(true)));
    }

    #[test]
    fn test_algebraic_simplification() {
        let mut opt = ExprOptimizer::new();

        // x + 0 = x
        let expr = Expr::binary(BinaryOp::Add, Expr::field("x"), Expr::literal(0));
        let optimized = opt.optimize(expr);
        assert_eq!(optimized, Expr::field("x"));

        opt.reset_stats();

        // x * 1 = x
        let expr = Expr::binary(BinaryOp::Mul, Expr::field("x"), Expr::literal(1));
        let optimized = opt.optimize(expr);
        assert_eq!(optimized, Expr::field("x"));

        opt.reset_stats();

        // x * 0 = 0
        let expr = Expr::binary(BinaryOp::Mul, Expr::field("x"), Expr::literal(0));
        let optimized = opt.optimize(expr);
        assert_eq!(optimized, Expr::Literal(Value::int(0)));
    }

    #[test]
    fn test_dead_code_elimination() {
        let mut opt = ExprOptimizer::new();

        // if true then A else B = A
        let expr = Expr::conditional(Expr::literal(true), Expr::field("a"), Expr::field("b"));
        let optimized = opt.optimize(expr);
        assert_eq!(optimized, Expr::field("a"));
        assert_eq!(opt.stats().dead_code_eliminations, 1);

        opt.reset_stats();

        // if false then A else B = B
        let expr = Expr::conditional(Expr::literal(false), Expr::field("a"), Expr::field("b"));
        let optimized = opt.optimize(expr);
        assert_eq!(optimized, Expr::field("b"));
    }

    #[test]
    fn test_unary_constant_folding() {
        let mut opt = ExprOptimizer::new();

        // !true = false
        let expr = Expr::unary(UnaryOp::Not, Expr::literal(true));
        let optimized = opt.optimize(expr);
        assert_eq!(optimized, Expr::Literal(Value::bool(false)));

        // -5 = -5
        let expr = Expr::unary(UnaryOp::Neg, Expr::literal(5));
        let optimized = opt.optimize(expr);
        assert_eq!(optimized, Expr::Literal(Value::int(-5)));
    }

    #[test]
    fn test_double_negation_elimination() {
        let mut opt = ExprOptimizer::new();

        // !!x = x
        let expr = Expr::unary(UnaryOp::Not, Expr::unary(UnaryOp::Not, Expr::field("x")));
        let optimized = opt.optimize(expr);
        assert_eq!(optimized, Expr::field("x"));

        // --x = x
        let expr = Expr::unary(UnaryOp::Neg, Expr::unary(UnaryOp::Neg, Expr::field("x")));
        let optimized = opt.optimize(expr);
        assert_eq!(optimized, Expr::field("x"));
    }

    #[test]
    fn test_nested_constant_folding() {
        let mut opt = ExprOptimizer::new();

        // price * (1 - 0.2) + 10 where price is a field
        // Should fold (1 - 0.2) = 0.8, and + 10 remains
        let inner = Expr::binary(BinaryOp::Sub, Expr::literal(1.0f64), Expr::literal(0.2f64));
        let mul = Expr::binary(BinaryOp::Mul, Expr::field("price"), inner);
        let expr = Expr::binary(BinaryOp::Add, mul, Expr::literal(10));

        let optimized = opt.optimize(expr);

        // Should be: price * 0.8 + 10
        if let Expr::Binary {
            op: BinaryOp::Add,
            left,
            right,
        } = optimized
        {
            if let Expr::Binary {
                op: BinaryOp::Mul,
                left: price,
                right: factor,
            } = *left
            {
                assert_eq!(*price, Expr::field("price"));
                if let Expr::Literal(Value::Float(f)) = *factor {
                    assert!((f - 0.8).abs() < 0.0001);
                } else {
                    panic!("Expected float literal for factor");
                }
            } else {
                panic!("Expected Mul expression");
            }
            assert_eq!(*right, Expr::Literal(Value::int(10)));
        } else {
            panic!("Expected Add expression");
        }
    }

    #[test]
    fn test_pure_function_folding() {
        let mut opt = ExprOptimizer::new();

        // len("hello") = 5
        let expr = Expr::call("len", vec![Expr::literal("hello")]);
        let optimized = opt.optimize(expr);
        assert_eq!(optimized, Expr::Literal(Value::int(5)));

        // upper("hello") = "HELLO"
        let expr = Expr::call("upper", vec![Expr::literal("hello")]);
        let optimized = opt.optimize(expr);
        assert_eq!(optimized, Expr::Literal(Value::string("HELLO")));

        // abs(-5) = 5
        let expr = Expr::call("abs", vec![Expr::literal(-5)]);
        let optimized = opt.optimize(expr);
        assert_eq!(optimized, Expr::Literal(Value::int(5)));
    }

    #[test]
    fn test_coalesce_optimization() {
        let mut opt = ExprOptimizer::new();

        // coalesce(null, "default") = "default"
        let expr = Expr::coalesce(vec![Expr::Literal(Value::Null), Expr::literal("default")]);
        let optimized = opt.optimize(expr);
        assert_eq!(optimized, Expr::Literal(Value::string("default")));

        // coalesce("value", "default") = "value"
        let expr = Expr::coalesce(vec![Expr::literal("value"), Expr::literal("default")]);
        let optimized = opt.optimize(expr);
        assert_eq!(optimized, Expr::Literal(Value::string("value")));
    }

    #[test]
    fn test_array_constant_folding() {
        let mut opt = ExprOptimizer::new();

        // [1, 2, 3] all literals
        let expr = Expr::Array(vec![Expr::literal(1), Expr::literal(2), Expr::literal(3)]);
        let optimized = opt.optimize(expr);

        if let Expr::Literal(Value::Array(arr)) = optimized {
            assert_eq!(arr.len(), 3);
            assert_eq!(arr[0], Value::int(1));
            assert_eq!(arr[1], Value::int(2));
            assert_eq!(arr[2], Value::int(3));
        } else {
            panic!("Expected literal array");
        }
    }
}
