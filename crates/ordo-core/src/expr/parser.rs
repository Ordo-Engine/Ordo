//! Expression parser
//!
//! Parses expression strings into AST

use super::ast::{BinaryOp, Expr, UnaryOp};
use crate::context::Value;
use crate::error::{OrdoError, Result};

/// Expression parser
pub struct ExprParser {
    input: Vec<char>,
    pos: usize,
}

impl ExprParser {
    /// Create a new parser
    pub fn new(input: &str) -> Self {
        Self {
            input: input.chars().collect(),
            pos: 0,
        }
    }

    /// Parse the input into an expression
    pub fn parse(input: &str) -> Result<Expr> {
        let mut parser = Self::new(input);
        let expr = parser.parse_expr()?;
        parser.skip_whitespace();
        if parser.pos < parser.input.len() {
            return Err(OrdoError::parse_error(format!(
                "Unexpected character at position {}: '{}'",
                parser.pos, parser.input[parser.pos]
            )));
        }
        Ok(expr)
    }

    /// Parse an expression
    fn parse_expr(&mut self) -> Result<Expr> {
        self.parse_or()
    }

    /// Parse OR expression (lowest precedence)
    fn parse_or(&mut self) -> Result<Expr> {
        let mut left = self.parse_and()?;

        while self.match_keyword("||") || self.match_keyword("or") {
            let right = self.parse_and()?;
            left = Expr::binary(BinaryOp::Or, left, right);
        }

        Ok(left)
    }

    /// Parse AND expression
    fn parse_and(&mut self) -> Result<Expr> {
        let mut left = self.parse_comparison()?;

        while self.match_keyword("&&") || self.match_keyword("and") {
            let right = self.parse_comparison()?;
            left = Expr::binary(BinaryOp::And, left, right);
        }

        Ok(left)
    }

    /// Parse comparison expression
    fn parse_comparison(&mut self) -> Result<Expr> {
        let mut left = self.parse_in()?;

        loop {
            self.skip_whitespace();
            let op = if self.match_str("==") {
                BinaryOp::Eq
            } else if self.match_str("!=") {
                BinaryOp::Ne
            } else if self.match_str("<=") {
                BinaryOp::Le
            } else if self.match_str(">=") {
                BinaryOp::Ge
            } else if self.match_str("<") {
                BinaryOp::Lt
            } else if self.match_str(">") {
                BinaryOp::Gt
            } else {
                break;
            };

            let right = self.parse_in()?;
            left = Expr::binary(op, left, right);
        }

        Ok(left)
    }

    /// Parse IN expression
    fn parse_in(&mut self) -> Result<Expr> {
        let left = self.parse_additive()?;

        self.skip_whitespace();
        if self.match_keyword("not") {
            self.skip_whitespace();
            if self.match_keyword("in") {
                let right = self.parse_additive()?;
                return Ok(Expr::binary(BinaryOp::NotIn, left, right));
            } else {
                return Err(OrdoError::parse_error("Expected 'in' after 'not'"));
            }
        } else if self.match_keyword("in") {
            let right = self.parse_additive()?;
            return Ok(Expr::binary(BinaryOp::In, left, right));
        } else if self.match_keyword("contains") {
            let right = self.parse_additive()?;
            return Ok(Expr::binary(BinaryOp::Contains, left, right));
        }

        Ok(left)
    }

    /// Parse additive expression (+, -)
    fn parse_additive(&mut self) -> Result<Expr> {
        let mut left = self.parse_multiplicative()?;

        loop {
            self.skip_whitespace();
            let op = if self.match_char('+') {
                BinaryOp::Add
            } else if self.match_char('-') {
                BinaryOp::Sub
            } else {
                break;
            };

            let right = self.parse_multiplicative()?;
            left = Expr::binary(op, left, right);
        }

        Ok(left)
    }

    /// Parse multiplicative expression (*, /, %)
    fn parse_multiplicative(&mut self) -> Result<Expr> {
        let mut left = self.parse_unary()?;

        loop {
            self.skip_whitespace();
            let op = if self.match_char('*') {
                BinaryOp::Mul
            } else if self.match_char('/') {
                BinaryOp::Div
            } else if self.match_char('%') {
                BinaryOp::Mod
            } else {
                break;
            };

            let right = self.parse_unary()?;
            left = Expr::binary(op, left, right);
        }

        Ok(left)
    }

    /// Parse unary expression (!, -)
    fn parse_unary(&mut self) -> Result<Expr> {
        self.skip_whitespace();

        if self.match_char('!') || self.match_keyword("not") {
            let operand = self.parse_unary()?;
            return Ok(Expr::unary(UnaryOp::Not, operand));
        }

        if self.match_char('-') {
            let operand = self.parse_unary()?;
            return Ok(Expr::unary(UnaryOp::Neg, operand));
        }

        self.parse_primary()
    }

    /// Parse primary expression (literals, fields, function calls, etc.)
    fn parse_primary(&mut self) -> Result<Expr> {
        self.skip_whitespace();

        // Parenthesized expression
        if self.match_char('(') {
            let expr = self.parse_expr()?;
            self.skip_whitespace();
            if !self.match_char(')') {
                return Err(OrdoError::parse_error("Expected ')'"));
            }
            return Ok(expr);
        }

        // Array literal
        if self.match_char('[') {
            return self.parse_array();
        }

        // Object literal
        if self.match_char('{') {
            return self.parse_object();
        }

        // String literal
        if self.peek() == Some('"') || self.peek() == Some('\'') {
            return self.parse_string();
        }

        // Number literal
        if self.peek().map(|c| c.is_ascii_digit()).unwrap_or(false) {
            return self.parse_number();
        }

        // Keywords and identifiers
        if self
            .peek()
            .map(|c| c.is_alphabetic() || c == '_' || c == '$')
            .unwrap_or(false)
        {
            return self.parse_identifier_or_keyword();
        }

        Err(OrdoError::parse_error(format!(
            "Unexpected character at position {}: {:?}",
            self.pos,
            self.peek()
        )))
    }

    /// Parse array literal
    fn parse_array(&mut self) -> Result<Expr> {
        let mut elements = Vec::new();

        self.skip_whitespace();
        if !self.check(']') {
            loop {
                let elem = self.parse_expr()?;
                elements.push(elem);
                self.skip_whitespace();
                if !self.match_char(',') {
                    break;
                }
            }
        }

        if !self.match_char(']') {
            return Err(OrdoError::parse_error("Expected ']'"));
        }

        Ok(Expr::Array(elements))
    }

    /// Parse object literal
    fn parse_object(&mut self) -> Result<Expr> {
        let mut pairs = Vec::new();

        self.skip_whitespace();
        if !self.check('}') {
            loop {
                self.skip_whitespace();
                let key = self.parse_string_value()?;
                self.skip_whitespace();
                if !self.match_char(':') {
                    return Err(OrdoError::parse_error("Expected ':' in object"));
                }
                let value = self.parse_expr()?;
                pairs.push((key, value));
                self.skip_whitespace();
                if !self.match_char(',') {
                    break;
                }
            }
        }

        if !self.match_char('}') {
            return Err(OrdoError::parse_error("Expected '}'"));
        }

        Ok(Expr::Object(pairs))
    }

    /// Parse string literal
    fn parse_string(&mut self) -> Result<Expr> {
        let s = self.parse_string_value()?;
        Ok(Expr::literal(s))
    }

    /// Parse string value (returns the string content)
    fn parse_string_value(&mut self) -> Result<String> {
        let quote = self
            .advance()
            .ok_or_else(|| OrdoError::parse_error("Expected string"))?;
        let mut s = String::new();

        while let Some(c) = self.peek() {
            if c == quote {
                self.advance();
                return Ok(s);
            }
            if c == '\\' {
                self.advance();
                match self.advance() {
                    Some('n') => s.push('\n'),
                    Some('t') => s.push('\t'),
                    Some('r') => s.push('\r'),
                    Some('\\') => s.push('\\'),
                    Some('"') => s.push('"'),
                    Some('\'') => s.push('\''),
                    Some(c) => s.push(c),
                    None => return Err(OrdoError::parse_error("Unexpected end of string")),
                }
            } else {
                s.push(c);
                self.advance();
            }
        }

        Err(OrdoError::parse_error("Unterminated string"))
    }

    /// Parse number literal
    fn parse_number(&mut self) -> Result<Expr> {
        let mut s = String::new();
        let mut is_float = false;

        while let Some(c) = self.peek() {
            if c.is_ascii_digit() {
                s.push(c);
                self.advance();
            } else if c == '.' && !is_float {
                // Check if it's a decimal point (not method call)
                if self.peek_at(1).map(|c| c.is_ascii_digit()).unwrap_or(false) {
                    is_float = true;
                    s.push(c);
                    self.advance();
                } else {
                    break;
                }
            } else {
                break;
            }
        }

        if is_float {
            let value: f64 = s
                .parse()
                .map_err(|_| OrdoError::parse_error(format!("Invalid float: {}", s)))?;
            Ok(Expr::literal(value))
        } else {
            let value: i64 = s
                .parse()
                .map_err(|_| OrdoError::parse_error(format!("Invalid integer: {}", s)))?;
            Ok(Expr::literal(value))
        }
    }

    /// Parse identifier or keyword
    fn parse_identifier_or_keyword(&mut self) -> Result<Expr> {
        let ident = self.parse_identifier()?;

        match ident.as_str() {
            "true" => return Ok(Expr::literal(true)),
            "false" => return Ok(Expr::literal(false)),
            "null" => return Ok(Expr::literal(Value::Null)),
            "exists" => {
                self.skip_whitespace();
                if !self.match_char('(') {
                    return Err(OrdoError::parse_error("Expected '(' after 'exists'"));
                }
                self.skip_whitespace();
                let path = if self.peek() == Some('"') || self.peek() == Some('\'') {
                    self.parse_string_value()?
                } else {
                    self.parse_identifier()?
                };
                self.skip_whitespace();
                if !self.match_char(')') {
                    return Err(OrdoError::parse_error("Expected ')' after exists path"));
                }
                return Ok(Expr::exists(path));
            }
            "coalesce" => {
                self.skip_whitespace();
                if !self.match_char('(') {
                    return Err(OrdoError::parse_error("Expected '(' after 'coalesce'"));
                }
                let mut args = Vec::new();
                loop {
                    let arg = self.parse_expr()?;
                    args.push(arg);
                    self.skip_whitespace();
                    if !self.match_char(',') {
                        break;
                    }
                }
                if !self.match_char(')') {
                    return Err(OrdoError::parse_error("Expected ')'"));
                }
                return Ok(Expr::coalesce(args));
            }
            "if" => {
                self.skip_whitespace();
                let condition = self.parse_expr()?;
                self.skip_whitespace();
                if !self.match_keyword("then") {
                    return Err(OrdoError::parse_error("Expected 'then' after condition"));
                }
                let then_branch = self.parse_expr()?;
                self.skip_whitespace();
                if !self.match_keyword("else") {
                    return Err(OrdoError::parse_error("Expected 'else' after then branch"));
                }
                let else_branch = self.parse_expr()?;
                return Ok(Expr::conditional(condition, then_branch, else_branch));
            }
            _ => {}
        }

        // Check for function call
        self.skip_whitespace();
        if self.match_char('(') {
            let args = self.parse_call_args()?;
            return Ok(Expr::call(ident, args));
        }

        // Field reference
        Ok(Expr::field(ident))
    }

    /// Parse function call arguments
    fn parse_call_args(&mut self) -> Result<Vec<Expr>> {
        let mut args = Vec::new();

        self.skip_whitespace();
        if !self.check(')') {
            loop {
                let arg = self.parse_expr()?;
                args.push(arg);
                self.skip_whitespace();
                if !self.match_char(',') {
                    break;
                }
            }
        }

        if !self.match_char(')') {
            return Err(OrdoError::parse_error("Expected ')'"));
        }

        Ok(args)
    }

    /// Parse an identifier (including dot-separated paths)
    fn parse_identifier(&mut self) -> Result<String> {
        let mut ident = String::new();

        // First character must be alphabetic, underscore, or dollar sign
        match self.peek() {
            Some(c) if c.is_alphabetic() || c == '_' || c == '$' => {
                ident.push(c);
                self.advance();
            }
            _ => return Err(OrdoError::parse_error("Expected identifier")),
        }

        // Rest can include alphanumeric, underscore, and dots
        while let Some(c) = self.peek() {
            if c.is_alphanumeric() || c == '_' || c == '.' {
                ident.push(c);
                self.advance();
            } else {
                break;
            }
        }

        Ok(ident)
    }

    // ==================== Helper methods ====================

    fn peek(&self) -> Option<char> {
        self.input.get(self.pos).copied()
    }

    fn peek_at(&self, offset: usize) -> Option<char> {
        self.input.get(self.pos + offset).copied()
    }

    fn advance(&mut self) -> Option<char> {
        let c = self.peek();
        if c.is_some() {
            self.pos += 1;
        }
        c
    }

    fn check(&self, c: char) -> bool {
        self.peek() == Some(c)
    }

    fn match_char(&mut self, c: char) -> bool {
        if self.check(c) {
            self.advance();
            true
        } else {
            false
        }
    }

    fn match_str(&mut self, s: &str) -> bool {
        let chars: Vec<char> = s.chars().collect();
        if self.input[self.pos..].starts_with(&chars) {
            self.pos += chars.len();
            true
        } else {
            false
        }
    }

    fn match_keyword(&mut self, keyword: &str) -> bool {
        self.skip_whitespace();
        let chars: Vec<char> = keyword.chars().collect();
        if self.input[self.pos..].starts_with(&chars) {
            // Check that keyword is not followed by alphanumeric
            let next_pos = self.pos + chars.len();
            if next_pos >= self.input.len() || !self.input[next_pos].is_alphanumeric() {
                self.pos = next_pos;
                return true;
            }
        }
        false
    }

    fn skip_whitespace(&mut self) {
        while let Some(c) = self.peek() {
            if c.is_whitespace() {
                self.advance();
            } else {
                break;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_literals() {
        assert_eq!(ExprParser::parse("42").unwrap(), Expr::literal(42i64));
        assert_eq!(ExprParser::parse("3.15").unwrap(), Expr::literal(3.15f64));
        assert_eq!(ExprParser::parse("true").unwrap(), Expr::literal(true));
        assert_eq!(
            ExprParser::parse("\"hello\"").unwrap(),
            Expr::literal("hello")
        );
    }

    #[test]
    fn test_parse_comparison() {
        let expr = ExprParser::parse("age > 18").unwrap();
        match expr {
            Expr::Binary {
                op: BinaryOp::Gt,
                left,
                right,
            } => {
                assert!(matches!(*left, Expr::Field(f) if f == "age"));
                assert!(matches!(*right, Expr::Literal(Value::Int(18))));
            }
            _ => panic!("Expected Binary Gt"),
        }
    }

    #[test]
    fn test_parse_logical() {
        let expr = ExprParser::parse("age > 18 && status == \"active\"").unwrap();
        assert!(matches!(
            expr,
            Expr::Binary {
                op: BinaryOp::And,
                ..
            }
        ));
    }

    #[test]
    fn test_parse_in() {
        let expr = ExprParser::parse("status in [\"active\", \"pending\"]").unwrap();
        assert!(matches!(
            expr,
            Expr::Binary {
                op: BinaryOp::In,
                ..
            }
        ));
    }

    #[test]
    fn test_parse_function() {
        let expr = ExprParser::parse("len(items) > 0").unwrap();
        match expr {
            Expr::Binary {
                op: BinaryOp::Gt,
                left,
                ..
            } => {
                assert!(matches!(*left, Expr::Call { name, .. } if name == "len"));
            }
            _ => panic!("Expected Binary Gt"),
        }
    }

    #[test]
    fn test_parse_exists() {
        let expr = ExprParser::parse("exists(user.premium)").unwrap();
        assert!(matches!(expr, Expr::Exists(p) if p == "user.premium"));
    }

    #[test]
    fn test_parse_if() {
        let expr = ExprParser::parse("if exists(discount) then price * 0.9 else price").unwrap();
        assert!(matches!(expr, Expr::Conditional { .. }));
    }

    #[test]
    fn test_parse_coalesce() {
        let expr = ExprParser::parse("coalesce(appid, in_appid)").unwrap();
        match expr {
            Expr::Coalesce(exprs) => {
                assert_eq!(exprs.len(), 2);
            }
            _ => panic!("Expected Coalesce"),
        }
    }
}
