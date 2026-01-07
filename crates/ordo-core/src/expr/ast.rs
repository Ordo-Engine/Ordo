//! Expression AST definitions
//!
//! Defines the abstract syntax tree for expressions

use crate::context::Value;
use serde::{Deserialize, Serialize};

/// Binary operators
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum BinaryOp {
    // Arithmetic
    Add, // +
    Sub, // -
    Mul, // *
    Div, // /
    Mod, // %

    // Comparison
    Eq, // ==
    Ne, // !=
    Lt, // <
    Le, // <=
    Gt, // >
    Ge, // >=

    // Logical
    And, // &&
    Or,  // ||

    // Set operations
    In,       // in
    NotIn,    // not in
    Contains, // contains
}

impl BinaryOp {
    /// Get operator precedence (higher = binds tighter)
    pub fn precedence(&self) -> u8 {
        match self {
            Self::Or => 1,
            Self::And => 2,
            Self::Eq | Self::Ne | Self::Lt | Self::Le | Self::Gt | Self::Ge => 3,
            Self::In | Self::NotIn | Self::Contains => 4,
            Self::Add | Self::Sub => 5,
            Self::Mul | Self::Div | Self::Mod => 6,
        }
    }

    /// Check if operator is right associative
    pub fn is_right_assoc(&self) -> bool {
        false
    }
}

/// Unary operators
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum UnaryOp {
    Not, // !
    Neg, // -
}

/// Expression AST node
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Expr {
    /// Literal value
    Literal(Value),

    /// Field reference (path like "user.name" or "$variable")
    Field(String),

    /// Binary operation
    Binary {
        op: BinaryOp,
        left: Box<Expr>,
        right: Box<Expr>,
    },

    /// Unary operation
    Unary { op: UnaryOp, operand: Box<Expr> },

    /// Function call
    Call { name: String, args: Vec<Expr> },

    /// Conditional expression (if-then-else)
    Conditional {
        condition: Box<Expr>,
        then_branch: Box<Expr>,
        else_branch: Box<Expr>,
    },

    /// Array literal
    Array(Vec<Expr>),

    /// Object literal
    Object(Vec<(String, Expr)>),

    /// Field existence check
    Exists(String),

    /// Coalesce (return first non-null value)
    Coalesce(Vec<Expr>),
}

impl Expr {
    // ==================== Constructors ====================

    /// Create a literal expression
    pub fn literal(value: impl Into<Value>) -> Self {
        Self::Literal(value.into())
    }

    /// Create a field reference
    pub fn field(path: impl Into<String>) -> Self {
        Self::Field(path.into())
    }

    /// Create a binary operation
    pub fn binary(op: BinaryOp, left: Expr, right: Expr) -> Self {
        Self::Binary {
            op,
            left: Box::new(left),
            right: Box::new(right),
        }
    }

    /// Create a unary operation
    pub fn unary(op: UnaryOp, operand: Expr) -> Self {
        Self::Unary {
            op,
            operand: Box::new(operand),
        }
    }

    /// Create a function call
    pub fn call(name: impl Into<String>, args: Vec<Expr>) -> Self {
        Self::Call {
            name: name.into(),
            args,
        }
    }

    /// Create a conditional expression
    pub fn conditional(condition: Expr, then_branch: Expr, else_branch: Expr) -> Self {
        Self::Conditional {
            condition: Box::new(condition),
            then_branch: Box::new(then_branch),
            else_branch: Box::new(else_branch),
        }
    }

    /// Create an exists check
    pub fn exists(path: impl Into<String>) -> Self {
        Self::Exists(path.into())
    }

    /// Create a coalesce expression
    pub fn coalesce(exprs: Vec<Expr>) -> Self {
        Self::Coalesce(exprs)
    }

    // ==================== Helpers ====================

    /// Create an equality comparison
    pub fn eq(left: Expr, right: Expr) -> Self {
        Self::binary(BinaryOp::Eq, left, right)
    }

    /// Create an inequality comparison
    pub fn ne(left: Expr, right: Expr) -> Self {
        Self::binary(BinaryOp::Ne, left, right)
    }

    /// Create a less-than comparison
    pub fn lt(left: Expr, right: Expr) -> Self {
        Self::binary(BinaryOp::Lt, left, right)
    }

    /// Create a less-than-or-equal comparison
    pub fn le(left: Expr, right: Expr) -> Self {
        Self::binary(BinaryOp::Le, left, right)
    }

    /// Create a greater-than comparison
    pub fn gt(left: Expr, right: Expr) -> Self {
        Self::binary(BinaryOp::Gt, left, right)
    }

    /// Create a greater-than-or-equal comparison
    pub fn ge(left: Expr, right: Expr) -> Self {
        Self::binary(BinaryOp::Ge, left, right)
    }

    /// Create a logical AND
    pub fn and(left: Expr, right: Expr) -> Self {
        Self::binary(BinaryOp::And, left, right)
    }

    /// Create a logical OR
    pub fn or(left: Expr, right: Expr) -> Self {
        Self::binary(BinaryOp::Or, left, right)
    }

    /// Create a logical NOT
    pub fn not(operand: Expr) -> Self {
        Self::unary(UnaryOp::Not, operand)
    }

    /// Create an "in" check
    pub fn is_in(value: Expr, collection: Expr) -> Self {
        Self::binary(BinaryOp::In, value, collection)
    }

    /// Create a "not in" check
    pub fn not_in(value: Expr, collection: Expr) -> Self {
        Self::binary(BinaryOp::NotIn, value, collection)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_expr_construction() {
        let expr = Expr::and(
            Expr::gt(Expr::field("age"), Expr::literal(18)),
            Expr::eq(Expr::field("status"), Expr::literal("active")),
        );

        match expr {
            Expr::Binary {
                op: BinaryOp::And,
                left,
                right,
            } => {
                assert!(matches!(
                    *left,
                    Expr::Binary {
                        op: BinaryOp::Gt,
                        ..
                    }
                ));
                assert!(matches!(
                    *right,
                    Expr::Binary {
                        op: BinaryOp::Eq,
                        ..
                    }
                ));
            }
            _ => panic!("Expected Binary And"),
        }
    }

    #[test]
    fn test_conditional_expr() {
        let expr = Expr::conditional(
            Expr::exists("premium"),
            Expr::literal(0.9),
            Expr::literal(1.0),
        );

        match expr {
            Expr::Conditional {
                condition,
                then_branch,
                else_branch,
            } => {
                assert!(matches!(*condition, Expr::Exists(_)));
                assert!(matches!(*then_branch, Expr::Literal(Value::Float(_))));
                assert!(matches!(*else_branch, Expr::Literal(Value::Float(_))));
            }
            _ => panic!("Expected Conditional"),
        }
    }
}
