//! A small expression parser/evaluator supporting:
//! - + - * / ^ with precedence
//! - parentheses
//! - unary +/-
//! - ln(x)
//! - log(x) (base 10)
//! - log(base, x)
//!
//! Examples:
//!   "2 + 3*4"        => 14
//!   "2^(1+2)"        => 8
//!   "-(3 + 4)"       => -7
//!   "ln(2.7182818)"  => ~1
//!   "log(100)"       => 2
//!   "log(2, 8)"      => 3

#[derive(Debug, Clone, PartialEq)]
pub enum Expr {
    Number(f64),
    Unary {
        op: UnaryOp,
        rhs: Box<Expr>,
    },
    Binary {
        op: BinOp,
        lhs: Box<Expr>,
        rhs: Box<Expr>,
    },
    Func {
        name: String,
        args: Vec<Expr>,
    },
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum UnaryOp {
    Plus,
    Minus,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum BinOp {
    Add,
    Sub,
    Mul,
    Div,
    Pow,
}

impl Expr {
    pub fn eval(&self) -> Option<f64> {
        use BinOp::*;
        use UnaryOp::*;
        match self {
            Expr::Number(x) => Some(*x),

            Expr::Unary { op, rhs } => {
                let v = rhs.eval()?;
                Some(match op {
                    Plus => v,
                    Minus => -v,
                })
            }

            Expr::Binary { op, lhs, rhs } => {
                let a = lhs.eval()?;
                let b = rhs.eval()?;
                match op {
                    Add => Some(a + b),
                    Sub => Some(a - b),
                    Mul => Some(a * b),
                    Div => Some(a / b),
                    Pow => Some(a.powf(b)),
                }
            }

            Expr::Func { name, args } => {
                let name = name.as_str();
                match name {
                    "ln" => {
                        if args.len() != 1 {
                            return None;
                        }
                        Some(args[0].eval()?.ln())
                    }
                    "log" => match args.len() {
                        1 => Some(args[0].eval()?.log10()),
                        2 => {
                            let base = args[0].eval()?;
                            let x = args[1].eval()?;
                            Some(x.log(base))
                        }
                        _ => None,
                    },
                    _ => None,
                }
            }
        }
    }

    pub fn from_str(s: &str) -> Result<Expr, String> {
        let mut p = Parser::new(s);
        let expr = p.parse_expr()?;
        p.expect(Token::End)?;
        Ok(expr)
    }
}

/* ---------------- Tokenizer ---------------- */

#[derive(Debug, Clone, PartialEq)]
enum Token {
    Number(f64),
    Ident(String),
    Plus,
    Minus,
    Star,
    Slash,
    Caret,
    LParen,
    RParen,
    Comma,
    End,
}

struct Lexer<'a> {
    input: &'a str,
    i: usize,
}

impl<'a> Lexer<'a> {
    fn new(input: &'a str) -> Self {
        Self { input, i: 0 }
    }

    fn peek_char(&self) -> Option<char> {
        self.input[self.i..].chars().next()
    }

    fn bump_char(&mut self) -> Option<char> {
        let c = self.peek_char()?;
        self.i += c.len_utf8();
        Some(c)
    }

    fn skip_ws(&mut self) {
        while matches!(self.peek_char(), Some(c) if c.is_whitespace()) {
            self.bump_char();
        }
    }

    fn next_token(&mut self) -> Result<Token, String> {
        self.skip_ws();
        let c = match self.peek_char() {
            Some(c) => c,
            None => return Ok(Token::End),
        };

        // single-char tokens
        let tok = match c {
            '+' => {
                self.bump_char();
                Token::Plus
            }
            '-' => {
                self.bump_char();
                Token::Minus
            }
            '*' => {
                self.bump_char();
                Token::Star
            }
            '/' => {
                self.bump_char();
                Token::Slash
            }
            '^' => {
                self.bump_char();
                Token::Caret
            }
            '(' => {
                self.bump_char();
                Token::LParen
            }
            ')' => {
                self.bump_char();
                Token::RParen
            }
            ',' => {
                self.bump_char();
                Token::Comma
            }
            _ => {
                // number or identifier
                if c.is_ascii_digit() || c == '.' {
                    return self.lex_number();
                } else if c.is_ascii_alphabetic() || c == '_' {
                    return self.lex_ident();
                } else {
                    return Err(format!("Unexpected character: {c}"));
                }
            }
        };
        Ok(tok)
    }

    fn lex_number(&mut self) -> Result<Token, String> {
        // Simple float lexer: digits/./e/E/+/- in exponent
        let start = self.i;
        let mut seen_e = false;

        while let Some(c) = self.peek_char() {
            if c.is_ascii_digit() || c == '.' {
                self.bump_char();
                continue;
            }
            if (c == 'e' || c == 'E') && !seen_e {
                seen_e = true;
                self.bump_char();
                // optional sign after exponent
                if matches!(self.peek_char(), Some('+' | '-')) {
                    self.bump_char();
                }
                continue;
            }
            break;
        }

        let s = &self.input[start..self.i];
        let n = s
            .parse::<f64>()
            .map_err(|_| format!("Invalid number: {s}"))?;
        Ok(Token::Number(n))
    }

    fn lex_ident(&mut self) -> Result<Token, String> {
        let start = self.i;
        while let Some(c) = self.peek_char() {
            if c.is_ascii_alphanumeric() || c == '_' {
                self.bump_char();
            } else {
                break;
            }
        }
        Ok(Token::Ident(self.input[start..self.i].to_string()))
    }
}

/* ---------------- Parser ---------------- */

struct Parser<'a> {
    lex: Lexer<'a>,
    cur: Token,
}

impl<'a> Parser<'a> {
    fn new(input: &'a str) -> Self {
        let mut lex = Lexer::new(input);
        let cur = lex.next_token().unwrap_or(Token::End);
        Self { lex, cur }
    }

    fn bump(&mut self) -> Result<(), String> {
        self.cur = self.lex.next_token()?;
        Ok(())
    }

    fn expect(&mut self, t: Token) -> Result<(), String> {
        if self.cur == t {
            self.bump()
        } else {
            Err(format!("Expected {:?}, found {:?}", t, self.cur))
        }
    }

    fn parse_expr(&mut self) -> Result<Expr, String> {
        // expr = term (('+'|'-') term)*
        let mut node = self.parse_term()?;
        loop {
            let op = match self.cur {
                Token::Plus => BinOp::Add,
                Token::Minus => BinOp::Sub,
                _ => break,
            };
            self.bump()?;
            let rhs = self.parse_term()?;
            node = Expr::Binary {
                op,
                lhs: Box::new(node),
                rhs: Box::new(rhs),
            };
        }
        Ok(node)
    }

    fn parse_term(&mut self) -> Result<Expr, String> {
        // term = power (('*'|'/') power)*
        let mut node = self.parse_power()?;
        loop {
            let op = match self.cur {
                Token::Star => BinOp::Mul,
                Token::Slash => BinOp::Div,
                _ => break,
            };
            self.bump()?;
            let rhs = self.parse_power()?;
            node = Expr::Binary {
                op,
                lhs: Box::new(node),
                rhs: Box::new(rhs),
            };
        }
        Ok(node)
    }

    fn parse_power(&mut self) -> Result<Expr, String> {
        // power = unary ('^' power)?  (right associative)
        let lhs = self.parse_unary()?;
        if self.cur == Token::Caret {
            self.bump()?;
            let rhs = self.parse_power()?;
            Ok(Expr::Binary {
                op: BinOp::Pow,
                lhs: Box::new(lhs),
                rhs: Box::new(rhs),
            })
        } else {
            Ok(lhs)
        }
    }

    fn parse_unary(&mut self) -> Result<Expr, String> {
        // unary = ('+'|'-')* primary
        match self.cur {
            Token::Plus => {
                self.bump()?;
                Ok(Expr::Unary {
                    op: UnaryOp::Plus,
                    rhs: Box::new(self.parse_unary()?),
                })
            }
            Token::Minus => {
                self.bump()?;
                Ok(Expr::Unary {
                    op: UnaryOp::Minus,
                    rhs: Box::new(self.parse_unary()?),
                })
            }
            _ => self.parse_primary(),
        }
    }

    fn parse_primary(&mut self) -> Result<Expr, String> {
        match &self.cur {
            Token::Number(n) => {
                let v = *n;
                self.bump()?;
                Ok(Expr::Number(v))
            }
            Token::LParen => {
                self.bump()?;
                let e = self.parse_expr()?;
                self.expect(Token::RParen)?;
                Ok(e)
            }
            Token::Ident(name) => {
                let name = name.clone();
                self.bump()?;
                // function call must be ident '(' ...
                self.expect(Token::LParen)?;
                let mut args = Vec::new();
                if self.cur != Token::RParen {
                    loop {
                        args.push(self.parse_expr()?);
                        if self.cur == Token::Comma {
                            self.bump()?;
                            continue;
                        }
                        break;
                    }
                }
                self.expect(Token::RParen)?;
                Ok(Expr::Func { name, args })
            }
            _ => Err(format!("Unexpected token: {:?}", self.cur)),
        }
    }
}
