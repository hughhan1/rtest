//! Boolean expression parser for `-k`.
//!
//! Compatibility target: pytest's `_pytest.mark.expression` (and/or/not, parentheses).
//! Nodeids may contain `::`, `[]`, and path separators; identifiers in expressions match
//! against collected keyword names via substring (case-insensitive).

#[derive(Debug, Clone, PartialEq)]
enum Token {
    Ident(String),
    Or,
    And,
    Not,
    LParen,
    RParen,
    Eof,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ParseError {
    pub column: usize,
    pub message: String,
}

impl std::fmt::Display for ParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "at column {}: {}", self.column, self.message)
    }
}

struct Scanner {
    input: Vec<char>,
    pos: usize,
}

impl Scanner {
    fn new(input: &str) -> Self {
        Self {
            input: input.chars().collect(),
            pos: 0,
        }
    }

    fn peek(&self) -> Option<char> {
        self.input.get(self.pos).copied()
    }

    fn advance(&mut self) -> Option<char> {
        let ch = self.peek()?;
        self.pos += 1;
        Some(ch)
    }

    fn skip_whitespace(&mut self) {
        while matches!(self.peek(), Some(' ' | '\t')) {
            self.advance();
        }
    }

    fn column(&self) -> usize {
        self.pos + 1
    }

    fn next_token(&mut self) -> Result<Token, ParseError> {
        self.skip_whitespace();
        let col = self.column();
        let Some(ch) = self.peek() else {
            return Ok(Token::Eof);
        };

        match ch {
            '(' => {
                self.advance();
                Ok(Token::LParen)
            }
            ')' => {
                self.advance();
                Ok(Token::RParen)
            }
            _ => {
                let start = self.pos;
                while let Some(c) = self.peek() {
                    if c.is_ascii_alphanumeric()
                        || c == '_'
                        || c == ':'
                        || c == '+'
                        || c == '-'
                        || c == '.'
                        || c == '['
                        || c == ']'
                        || c == '\\'
                        || c == '/'
                    {
                        self.advance();
                    } else if c.is_whitespace() || c == '(' || c == ')' {
                        break;
                    } else {
                        return Err(ParseError {
                            column: self.column(),
                            message: format!("unexpected character \"{c}\""),
                        });
                    }
                }
                let value: String = self.input[start..self.pos].iter().collect();
                if value.is_empty() {
                    return Err(ParseError {
                        column: col,
                        message: format!("unexpected character \"{ch}\""),
                    });
                }
                match value.as_str() {
                    "or" => Ok(Token::Or),
                    "and" => Ok(Token::And),
                    "not" => Ok(Token::Not),
                    _ => Ok(Token::Ident(value)),
                }
            }
        }
    }
}

struct Parser {
    current: Token,
    scanner: Scanner,
}

impl Parser {
    fn new(input: &str) -> Result<Self, ParseError> {
        let mut scanner = Scanner::new(input);
        let current = scanner.next_token()?;
        Ok(Self { current, scanner })
    }

    fn advance(&mut self) -> Result<(), ParseError> {
        self.current = self.scanner.next_token()?;
        Ok(())
    }

    fn accept(&mut self, token: &Token) -> bool {
        if &self.current == token {
            let _ = self.advance();
            true
        } else {
            false
        }
    }

    fn expect(&mut self, token: &Token) -> Result<(), ParseError> {
        if self.accept(token) {
            Ok(())
        } else {
            Err(ParseError {
                column: self.scanner.column(),
                message: format!("expected {token:?}; got {:?}", self.current),
            })
        }
    }

    fn parse(mut self) -> Result<Expr, ParseError> {
        if matches!(self.current, Token::Eof) {
            return Ok(Expr::Const(false));
        }
        let expr = self.parse_or()?;
        self.expect(&Token::Eof)?;
        Ok(expr)
    }

    fn parse_or(&mut self) -> Result<Expr, ParseError> {
        let mut left = self.parse_and()?;
        while self.accept(&Token::Or) {
            let right = self.parse_and()?;
            left = Expr::Or(Box::new(left), Box::new(right));
        }
        Ok(left)
    }

    fn parse_and(&mut self) -> Result<Expr, ParseError> {
        let mut left = self.parse_not()?;
        while self.accept(&Token::And) {
            let right = self.parse_not()?;
            left = Expr::And(Box::new(left), Box::new(right));
        }
        Ok(left)
    }

    fn parse_not(&mut self) -> Result<Expr, ParseError> {
        if self.accept(&Token::Not) {
            Ok(Expr::Not(Box::new(self.parse_not()?)))
        } else if self.accept(&Token::LParen) {
            let inner = self.parse_or()?;
            self.expect(&Token::RParen)?;
            Ok(inner)
        } else if let Token::Ident(name) = self.current.clone() {
            self.advance()?;
            Ok(Expr::Ident(name))
        } else {
            Err(ParseError {
                column: self.scanner.column(),
                message: format!("expected not, (, or identifier; got {:?}", self.current),
            })
        }
    }
}

#[derive(Debug, Clone)]
enum Expr {
    Const(bool),
    Ident(String),
    Or(Box<Expr>, Box<Expr>),
    And(Box<Expr>, Box<Expr>),
    Not(Box<Expr>),
}

impl Expr {
    fn eval<M: Fn(&str) -> bool>(&self, matcher: &M) -> bool {
        match self {
            Expr::Const(v) => *v,
            Expr::Ident(name) => matcher(name),
            Expr::Or(a, b) => a.eval(matcher) || b.eval(matcher),
            Expr::And(a, b) => a.eval(matcher) && b.eval(matcher),
            Expr::Not(a) => !a.eval(matcher),
        }
    }
}

/// Parsed `-k` expression for repeated evaluation.
#[derive(Debug, Clone)]
pub struct CompiledExpression(Expr);

/// Parse a `-k` expression once for repeated evaluation.
pub fn compile_expression(input: &str) -> Result<CompiledExpression, ParseError> {
    let parser = Parser::new(input)?;
    parser.parse().map(CompiledExpression)
}

/// Evaluate a compiled `-k` expression against a keyword matcher function.
pub fn evaluate_compiled(expr: &CompiledExpression, matcher: impl Fn(&str) -> bool) -> bool {
    expr.0.eval(&matcher)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn eval(input: &str, matcher: impl Fn(&str) -> bool) -> bool {
        let expr = compile_expression(input).unwrap();
        evaluate_compiled(&expr, matcher)
    }

    #[test]
    fn test_empty_expression_is_false() {
        assert!(!eval("", |_| true));
    }

    #[test]
    fn test_and_or_not() {
        let m = |name: &str| name == "foo" || name == "bar";
        assert!(eval("foo", m));
        assert!(eval("foo and bar", m));
        assert!(eval("foo or baz", m));
        assert!(!eval("not foo", m));
    }
}
