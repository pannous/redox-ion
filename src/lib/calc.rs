//! Pure Rust arithmetic calculator for ion shell.
//!
//! Replaces the `calculate` crate which depends on `decimal` (C code).
//! Uses f64 for calculations which is sufficient for shell scripting.

use std::collections::HashMap;
use std::fmt;

/// Represents a calculated value - either floating point or integer.
#[derive(Debug, Clone)]
pub enum Value {
    /// A floating-point value.
    Float(f64),
    /// An integer value.
    Int(i64),
}

impl fmt::Display for Value {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Value::Float(v) => {
                if v.fract() == 0.0 && v.abs() < 1e15 {
                    write!(f, "{}", *v as i64)
                } else {
                    write!(f, "{}", v)
                }
            }
            Value::Int(v) => write!(f, "{}", v),
        }
    }
}

/// Error type for calculator operations.
#[derive(Debug, Clone)]
pub struct CalcError(
    /// The error message.
    pub String,
);

impl fmt::Display for CalcError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl std::error::Error for CalcError {}

/// Parsing and evaluation environment.
pub mod parse {
    use super::*;

    /// Trait for variable environments used during evaluation.
    pub trait Environment {
        /// Get a variable's value by name.
        fn get(&self, name: &str) -> Option<f64>;
        /// Set a variable's value.
        fn set(&mut self, name: &str, value: f64);
        /// Set the "ans" (last answer) value.
        fn set_ans(&mut self, value: Option<Value>);
    }

    /// Default implementation of Environment with variable storage.
    #[derive(Default)]
    pub struct DefaultEnvironment {
        vars: HashMap<String, f64>,
        ans: Option<Value>,
    }

    impl DefaultEnvironment {
        /// Create a new empty environment.
        pub fn new() -> Self {
            Self::default()
        }

        /// Create a new environment with an initial "ans" value.
        pub fn with_ans(ans: Option<Value>) -> Self {
            Self { vars: HashMap::new(), ans }
        }
    }

    impl Environment for DefaultEnvironment {
        fn get(&self, name: &str) -> Option<f64> {
            if name == "ans" {
                return self.ans.as_ref().map(|v| match v {
                    Value::Float(f) => *f,
                    Value::Int(i) => *i as f64,
                });
            }
            self.vars.get(name).copied()
        }

        fn set(&mut self, name: &str, value: f64) {
            self.vars.insert(name.to_string(), value);
        }

        fn set_ans(&mut self, value: Option<Value>) {
            self.ans = value;
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
enum Token {
    Number(f64),
    Ident(String),
    Plus,
    Minus,
    Star,
    Slash,
    Percent,
    Caret,
    LParen,
    RParen,
}

fn tokenize(input: &str) -> Result<Vec<Token>, CalcError> {
    let mut tokens = Vec::new();
    let mut chars = input.chars().peekable();

    while let Some(&c) = chars.peek() {
        match c {
            ' ' | '\t' | '\n' | '\r' => { chars.next(); }
            '+' => { tokens.push(Token::Plus); chars.next(); }
            '-' => { tokens.push(Token::Minus); chars.next(); }
            '*' => {
                chars.next();
                if chars.peek() == Some(&'*') {
                    chars.next();
                    tokens.push(Token::Caret);
                } else {
                    tokens.push(Token::Star);
                }
            }
            '/' => { tokens.push(Token::Slash); chars.next(); }
            '%' => { tokens.push(Token::Percent); chars.next(); }
            '^' => { tokens.push(Token::Caret); chars.next(); }
            '(' => { tokens.push(Token::LParen); chars.next(); }
            ')' => { tokens.push(Token::RParen); chars.next(); }
            '0'..='9' | '.' => {
                let mut num = String::new();
                while let Some(&c) = chars.peek() {
                    if c.is_ascii_digit() || c == '.' || c == 'e' || c == 'E' {
                        num.push(c);
                        chars.next();
                        if (c == 'e' || c == 'E') && chars.peek() == Some(&'-') {
                            num.push('-');
                            chars.next();
                        }
                    } else {
                        break;
                    }
                }
                let n = num.parse::<f64>().map_err(|_| CalcError(format!("Invalid number: {}", num)))?;
                tokens.push(Token::Number(n));
            }
            'a'..='z' | 'A'..='Z' | '_' => {
                let mut ident = String::new();
                while let Some(&c) = chars.peek() {
                    if c.is_alphanumeric() || c == '_' {
                        ident.push(c);
                        chars.next();
                    } else {
                        break;
                    }
                }
                tokens.push(Token::Ident(ident));
            }
            _ => return Err(CalcError(format!("Unexpected character: {}", c))),
        }
    }

    Ok(tokens)
}

struct Parser<'a, E: parse::Environment> {
    tokens: Vec<Token>,
    pos: usize,
    env: &'a mut E,
}

impl<'a, E: parse::Environment> Parser<'a, E> {
    fn new(tokens: Vec<Token>, env: &'a mut E) -> Self {
        Self { tokens, pos: 0, env }
    }

    fn peek(&self) -> Option<&Token> {
        self.tokens.get(self.pos)
    }

    fn advance(&mut self) -> Option<Token> {
        if self.pos < self.tokens.len() {
            let tok = self.tokens[self.pos].clone();
            self.pos += 1;
            Some(tok)
        } else {
            None
        }
    }

    fn parse(&mut self) -> Result<f64, CalcError> {
        let result = self.expr()?;
        if self.pos < self.tokens.len() {
            return Err(CalcError(format!("Unexpected token: {:?}", self.tokens[self.pos])));
        }
        Ok(result)
    }

    fn expr(&mut self) -> Result<f64, CalcError> {
        self.add_sub()
    }

    fn add_sub(&mut self) -> Result<f64, CalcError> {
        let mut left = self.mul_div()?;
        while let Some(tok) = self.peek() {
            match tok {
                Token::Plus => { self.advance(); left += self.mul_div()?; }
                Token::Minus => { self.advance(); left -= self.mul_div()?; }
                _ => break,
            }
        }
        Ok(left)
    }

    fn mul_div(&mut self) -> Result<f64, CalcError> {
        let mut left = self.power()?;
        while let Some(tok) = self.peek() {
            match tok {
                Token::Star => { self.advance(); left *= self.power()?; }
                Token::Slash => {
                    self.advance();
                    let right = self.power()?;
                    if right == 0.0 { return Err(CalcError("Division by zero".into())); }
                    left /= right;
                }
                Token::Percent => {
                    self.advance();
                    let right = self.power()?;
                    if right == 0.0 { return Err(CalcError("Modulo by zero".into())); }
                    left %= right;
                }
                _ => break,
            }
        }
        Ok(left)
    }

    fn power(&mut self) -> Result<f64, CalcError> {
        let base = self.unary()?;
        if let Some(Token::Caret) = self.peek() {
            self.advance();
            let exp = self.power()?;
            Ok(base.powf(exp))
        } else {
            Ok(base)
        }
    }

    fn unary(&mut self) -> Result<f64, CalcError> {
        match self.peek() {
            Some(Token::Plus) => { self.advance(); self.unary() }
            Some(Token::Minus) => { self.advance(); Ok(-self.unary()?) }
            _ => self.call(),
        }
    }

    fn call(&mut self) -> Result<f64, CalcError> {
        if let Some(Token::Ident(name)) = self.peek().cloned() {
            let name_lower = name.to_lowercase();
            if let Some(Token::LParen) = self.tokens.get(self.pos + 1) {
                self.advance();
                self.advance();
                let arg = self.expr()?;
                if self.peek() != Some(&Token::RParen) {
                    return Err(CalcError("Expected ')'".into()));
                }
                self.advance();
                return self.apply_func(&name_lower, arg);
            }
        }
        self.primary()
    }

    fn apply_func(&self, name: &str, arg: f64) -> Result<f64, CalcError> {
        Ok(match name {
            "sin" => arg.sin(),
            "cos" => arg.cos(),
            "tan" => arg.tan(),
            "asin" => arg.asin(),
            "acos" => arg.acos(),
            "atan" => arg.atan(),
            "sinh" => arg.sinh(),
            "cosh" => arg.cosh(),
            "tanh" => arg.tanh(),
            "sqrt" => arg.sqrt(),
            "cbrt" => arg.cbrt(),
            "abs" => arg.abs(),
            "floor" => arg.floor(),
            "ceil" => arg.ceil(),
            "round" => arg.round(),
            "trunc" => arg.trunc(),
            "ln" => arg.ln(),
            "log" | "log10" => arg.log10(),
            "log2" => arg.log2(),
            "exp" => arg.exp(),
            _ => return Err(CalcError(format!("Unknown function: {}", name))),
        })
    }

    fn primary(&mut self) -> Result<f64, CalcError> {
        match self.peek().cloned() {
            Some(Token::Number(n)) => { self.advance(); Ok(n) }
            Some(Token::Ident(name)) => {
                self.advance();
                let name_lower = name.to_lowercase();
                match name_lower.as_str() {
                    "pi" => Ok(std::f64::consts::PI),
                    "e" => Ok(std::f64::consts::E),
                    "tau" => Ok(std::f64::consts::TAU),
                    _ => self.env.get(&name).ok_or_else(|| CalcError(format!("Unknown variable: {}", name))),
                }
            }
            Some(Token::LParen) => {
                self.advance();
                let val = self.expr()?;
                if self.peek() != Some(&Token::RParen) {
                    return Err(CalcError("Expected ')'".into()));
                }
                self.advance();
                Ok(val)
            }
            Some(tok) => Err(CalcError(format!("Unexpected token: {:?}", tok))),
            None => Err(CalcError("Unexpected end of expression".into())),
        }
    }
}

/// Evaluate an arithmetic expression using a default environment.
pub fn eval(input: &str) -> Result<Value, CalcError> {
    let mut env = parse::DefaultEnvironment::new();
    eval_with_env(input, &mut env)
}

/// Evaluate an arithmetic expression with a custom environment for variables.
pub fn eval_with_env(input: &str, env: &mut impl parse::Environment) -> Result<Value, CalcError> {
    if input.trim().is_empty() {
        return Err(CalcError("Empty expression".into()));
    }
    let tokens = tokenize(input)?;
    if tokens.is_empty() {
        return Err(CalcError("Empty expression".into()));
    }
    let mut parser = Parser::new(tokens, env);
    let result = parser.parse()?;
    Ok(Value::Float(result))
}

/// Evaluate an expression in Polish (prefix) notation with a custom environment.
pub fn eval_polish_with_env(input: &str, env: &mut impl parse::Environment) -> Result<Value, CalcError> {
    let tokens: Vec<&str> = input.split_whitespace().collect();
    let result = eval_polish_recursive(&tokens, &mut 0, env)?;
    Ok(Value::Float(result))
}

fn eval_polish_recursive(tokens: &[&str], pos: &mut usize, env: &impl parse::Environment) -> Result<f64, CalcError> {
    if *pos >= tokens.len() {
        return Err(CalcError("Unexpected end of expression".into()));
    }
    let token = tokens[*pos];
    *pos += 1;

    if let Ok(n) = token.parse::<f64>() {
        return Ok(n);
    }
    if let Some(val) = env.get(token) {
        return Ok(val);
    }

    match token {
        "+" => {
            let a = eval_polish_recursive(tokens, pos, env)?;
            let b = eval_polish_recursive(tokens, pos, env)?;
            Ok(a + b)
        }
        "-" => {
            let a = eval_polish_recursive(tokens, pos, env)?;
            let b = eval_polish_recursive(tokens, pos, env)?;
            Ok(a - b)
        }
        "*" => {
            let a = eval_polish_recursive(tokens, pos, env)?;
            let b = eval_polish_recursive(tokens, pos, env)?;
            Ok(a * b)
        }
        "/" => {
            let a = eval_polish_recursive(tokens, pos, env)?;
            let b = eval_polish_recursive(tokens, pos, env)?;
            if b == 0.0 { return Err(CalcError("Division by zero".into())); }
            Ok(a / b)
        }
        "%" => {
            let a = eval_polish_recursive(tokens, pos, env)?;
            let b = eval_polish_recursive(tokens, pos, env)?;
            if b == 0.0 { return Err(CalcError("Modulo by zero".into())); }
            Ok(a % b)
        }
        "^" | "**" => {
            let a = eval_polish_recursive(tokens, pos, env)?;
            let b = eval_polish_recursive(tokens, pos, env)?;
            Ok(a.powf(b))
        }
        "sin" => Ok(eval_polish_recursive(tokens, pos, env)?.sin()),
        "cos" => Ok(eval_polish_recursive(tokens, pos, env)?.cos()),
        "tan" => Ok(eval_polish_recursive(tokens, pos, env)?.tan()),
        "sqrt" => Ok(eval_polish_recursive(tokens, pos, env)?.sqrt()),
        "ln" => Ok(eval_polish_recursive(tokens, pos, env)?.ln()),
        "log" => Ok(eval_polish_recursive(tokens, pos, env)?.log10()),
        "exp" => Ok(eval_polish_recursive(tokens, pos, env)?.exp()),
        "abs" => Ok(eval_polish_recursive(tokens, pos, env)?.abs()),
        "pi" => Ok(std::f64::consts::PI),
        "e" => Ok(std::f64::consts::E),
        _ => Err(CalcError(format!("Unknown token: {}", token))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basic_math() {
        assert_eq!(eval("2 + 3").unwrap().to_string(), "5");
        assert_eq!(eval("10 - 4").unwrap().to_string(), "6");
        assert_eq!(eval("3 * 4").unwrap().to_string(), "12");
        assert_eq!(eval("15 / 3").unwrap().to_string(), "5");
    }

    #[test]
    fn test_precedence() {
        assert_eq!(eval("2 + 3 * 4").unwrap().to_string(), "14");
        assert_eq!(eval("(2 + 3) * 4").unwrap().to_string(), "20");
    }

    #[test]
    fn test_power() {
        assert_eq!(eval("2 ^ 3").unwrap().to_string(), "8");
        assert_eq!(eval("2 ** 3").unwrap().to_string(), "8");
    }

    #[test]
    fn test_functions() {
        assert_eq!(eval("sqrt(16)").unwrap().to_string(), "4");
        assert_eq!(eval("abs(-5)").unwrap().to_string(), "5");
    }

    #[test]
    fn test_polish() {
        let mut env = parse::DefaultEnvironment::new();
        assert_eq!(eval_polish_with_env("+ 2 3", &mut env).unwrap().to_string(), "5");
        assert_eq!(eval_polish_with_env("* + 2 3 4", &mut env).unwrap().to_string(), "20");
    }
}
