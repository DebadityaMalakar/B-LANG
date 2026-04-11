use crate::error::{Error, SourceLocation};

#[derive(Clone, Debug, PartialEq)]
pub enum Keyword {
    Auto,
    Extrn,
    If,
    Else,
    While,
    Switch,
    Case,
    Default,
    Break,
    Return,
    Goto,
    Include,
    Use,
    Namespace,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Symbol {
    Plus,
    Minus,
    Star,
    Slash,
    Percent,
    LShift,
    RShift,
    Lt,
    Le,
    Gt,
    Ge,
    Eq,
    Ne,
    BitAnd,
    BitOr,
    BitXor,
    Not,
    BitNot,
    Assign,
    PlusAssign,
    MinusAssign,
    StarAssign,
    SlashAssign,
    PercentAssign,
    AndAssign,
    OrAssign,
    XorAssign,
    LShiftAssign,
    RShiftAssign,
    AndAnd,
    OrOr,
    PlusPlus,
    MinusMinus,
    Question,
    Colon,
    ColonColon,
    Comma,
    Semi,
    LParen,
    RParen,
    LBrace,
    RBrace,
    LBracket,
    RBracket,
}

#[derive(Clone, Debug, PartialEq)]
pub enum TokenKind {
    Ident(String),
    Number(i64),
    CharConst(i64),
    StringLit(String),
    Keyword(Keyword),
    Symbol(Symbol),
    Eof,
}

#[derive(Clone, Debug)]
pub struct Token {
    pub kind: TokenKind,
    pub location: SourceLocation,
}

pub fn lex(source: &str) -> Result<Vec<Token>, Error> {
    let mut lexer = Lexer::new(source);
    let mut tokens = Vec::new();
    loop {
        let token = lexer.next_token()?;
        let is_eof = token.kind == TokenKind::Eof;
        tokens.push(token);
        if is_eof {
            break;
        }
    }
    Ok(tokens)
}

struct Lexer {
    chars: Vec<char>,
    index: usize,
    line: usize,
    column: usize,
}

impl Lexer {
    fn new(source: &str) -> Self {
        Self {
            chars: source.chars().collect(),
            index: 0,
            line: 1,
            column: 1,
        }
    }

    fn next_token(&mut self) -> Result<Token, Error> {
        self.skip_whitespace_and_comments()?;
        let location = self.location();
        let ch = match self.peek_char() {
            Some(c) => c,
            None => {
                return Ok(Token {
                    kind: TokenKind::Eof,
                    location,
                })
            }
        };

        if Self::is_ident_start(ch) {
            let ident = self.read_identifier();
            let kind = match ident.as_str() {
                "auto" => TokenKind::Keyword(Keyword::Auto),
                "extrn" => TokenKind::Keyword(Keyword::Extrn),
                "if" => TokenKind::Keyword(Keyword::If),
                "else" => TokenKind::Keyword(Keyword::Else),
                "while" => TokenKind::Keyword(Keyword::While),
                "switch" => TokenKind::Keyword(Keyword::Switch),
                "case" => TokenKind::Keyword(Keyword::Case),
                "default" => TokenKind::Keyword(Keyword::Default),
                "break" => TokenKind::Keyword(Keyword::Break),
                "return" => TokenKind::Keyword(Keyword::Return),
                "goto" => TokenKind::Keyword(Keyword::Goto),
                "include" => TokenKind::Keyword(Keyword::Include),
                "use" => TokenKind::Keyword(Keyword::Use),
                "namespace" => TokenKind::Keyword(Keyword::Namespace),
                _ => TokenKind::Ident(ident),
            };
            return Ok(Token { kind, location });
        }

        if ch.is_ascii_digit() {
            let number = self.read_number()?;
            return Ok(Token {
                kind: TokenKind::Number(number),
                location,
            });
        }

        match ch {
            '\'' => {
                self.advance_char();
                let value = self.read_char_const()?;
                self.expect_char('\'')?;
                return Ok(Token {
                    kind: TokenKind::CharConst(value),
                    location,
                });
            }
            '"' => {
                self.advance_char();
                let value = self.read_string_literal()?;
                return Ok(Token {
                    kind: TokenKind::StringLit(value),
                    location,
                });
            }
            _ => {}
        }

        let symbol = self.read_symbol()?;
        Ok(Token {
            kind: TokenKind::Symbol(symbol),
            location,
        })
    }

    fn location(&self) -> SourceLocation {
        SourceLocation {
            line: self.line,
            column: self.column,
        }
    }

    fn skip_whitespace_and_comments(&mut self) -> Result<(), Error> {
        loop {
            let mut advanced = false;
            while let Some(ch) = self.peek_char() {
                if ch == ' ' || ch == '\t' || ch == '\r' || ch == '\n' {
                    self.advance_char();
                    advanced = true;
                } else {
                    break;
                }
            }

            if self.peek_char() == Some('/') && self.peek_next_char() == Some('*') {
                advanced = true;
                self.advance_char();
                self.advance_char();
                loop {
                    match self.peek_char() {
                        Some('*') if self.peek_next_char() == Some('/') => {
                            self.advance_char();
                            self.advance_char();
                            break;
                        }
                        Some(_) => {
                            self.advance_char();
                        }
                        None => {
                            return Err(Error::lex(
                                "unterminated comment",
                                Some(self.location()),
                            ))
                        }
                    }
                }
            }

            if !advanced {
                break;
            }
        }
        Ok(())
    }

    fn read_identifier(&mut self) -> String {
        let mut ident = String::new();
        while let Some(ch) = self.peek_char() {
            if Self::is_ident_continue(ch) {
                ident.push(ch);
                self.advance_char();
            } else {
                break;
            }
        }
        ident
    }

    fn read_number(&mut self) -> Result<i64, Error> {
        let mut digits = String::new();
        while let Some(ch) = self.peek_char() {
            if ch.is_ascii_digit() {
                digits.push(ch);
                self.advance_char();
            } else {
                break;
            }
        }

        if digits.starts_with('0') && digits.len() > 1 {
            i64::from_str_radix(&digits, 8).map_err(|_| {
                Error::lex("invalid octal literal", Some(self.location()))
            })
        } else {
            digits.parse::<i64>().map_err(|_| {
                Error::lex("invalid number literal", Some(self.location()))
            })
        }
    }

    fn read_char_const(&mut self) -> Result<i64, Error> {
        let ch = match self.peek_char() {
            Some(c) => c,
            None => {
                return Err(Error::lex(
                    "unterminated character literal",
                    Some(self.location()),
                ))
            }
        };
        let value = if ch == '\\' {
            self.advance_char();
            self.read_escape()? as i64
        } else {
            self.advance_char();
            ch as u8 as i64
        };
        Ok(value)
    }

    fn read_string_literal(&mut self) -> Result<String, Error> {
        let mut result = String::new();
        loop {
            match self.peek_char() {
                Some('"') => {
                    self.advance_char();
                    break;
                }
                Some('\\') => {
                    self.advance_char();
                    let esc = self.read_escape()?;
                    result.push(esc as char);
                }
                Some(ch) => {
                    self.advance_char();
                    result.push(ch);
                }
                None => {
                    return Err(Error::lex(
                        "unterminated string literal",
                        Some(self.location()),
                    ))
                }
            }
        }
        Ok(result)
    }

    fn read_escape(&mut self) -> Result<u8, Error> {
        if self.peek_char() != Some('*') {
            return Err(Error::lex(
                "invalid escape sequence",
                Some(self.location()),
            ));
        }
        self.advance_char();
        let esc = match self.peek_char() {
            Some('n') => b'\n',
            Some('t') => b'\t',
            Some('e') => 4,
            Some('0') => 0,
            Some('"') => b'"',
            Some('\'') => b'\'',
            Some('*') => b'*',
            Some(ch) => {
                return Err(Error::lex(
                    format!("unknown escape *{}", ch),
                    Some(self.location()),
                ))
            }
            None => {
                return Err(Error::lex(
                    "unterminated escape sequence",
                    Some(self.location()),
                ))
            }
        };
        self.advance_char();
        Ok(esc)
    }

    fn read_symbol(&mut self) -> Result<Symbol, Error> {
        let location = self.location();
        let ch = match self.peek_char() {
            Some(ch) => ch,
            None => {
                return Err(Error::lex(
                    "unexpected end of input",
                    Some(location),
                ))
            }
        };

        let symbol = match ch {
            '+' => {
                self.advance_char();
                match self.peek_char() {
                    Some('+') => {
                        self.advance_char();
                        Symbol::PlusPlus
                    }
                    Some('=') => {
                        self.advance_char();
                        Symbol::PlusAssign
                    }
                    _ => Symbol::Plus,
                }
            }
            '-' => {
                self.advance_char();
                match self.peek_char() {
                    Some('-') => {
                        self.advance_char();
                        Symbol::MinusMinus
                    }
                    Some('=') => {
                        self.advance_char();
                        Symbol::MinusAssign
                    }
                    _ => Symbol::Minus,
                }
            }
            '*' => {
                self.advance_char();
                if self.peek_char() == Some('=') {
                    self.advance_char();
                    Symbol::StarAssign
                } else {
                    Symbol::Star
                }
            }
            '/' => {
                self.advance_char();
                if self.peek_char() == Some('=') {
                    self.advance_char();
                    Symbol::SlashAssign
                } else {
                    Symbol::Slash
                }
            }
            '%' => {
                self.advance_char();
                if self.peek_char() == Some('=') {
                    self.advance_char();
                    Symbol::PercentAssign
                } else {
                    Symbol::Percent
                }
            }
            '<' => {
                self.advance_char();
                match self.peek_char() {
                    Some('<') => {
                        self.advance_char();
                        if self.peek_char() == Some('=') {
                            self.advance_char();
                            Symbol::LShiftAssign
                        } else {
                            Symbol::LShift
                        }
                    }
                    Some('=') => {
                        self.advance_char();
                        Symbol::Le
                    }
                    _ => Symbol::Lt,
                }
            }
            '>' => {
                self.advance_char();
                match self.peek_char() {
                    Some('>') => {
                        self.advance_char();
                        if self.peek_char() == Some('=') {
                            self.advance_char();
                            Symbol::RShiftAssign
                        } else {
                            Symbol::RShift
                        }
                    }
                    Some('=') => {
                        self.advance_char();
                        Symbol::Ge
                    }
                    _ => Symbol::Gt,
                }
            }
            '=' => {
                self.advance_char();
                if self.peek_char() == Some('=') {
                    self.advance_char();
                    Symbol::Eq
                } else {
                    Symbol::Assign
                }
            }
            '!' => {
                self.advance_char();
                if self.peek_char() == Some('=') {
                    self.advance_char();
                    Symbol::Ne
                } else {
                    Symbol::Not
                }
            }
            '&' => {
                self.advance_char();
                match self.peek_char() {
                    Some('&') => {
                        self.advance_char();
                        Symbol::AndAnd
                    }
                    Some('=') => {
                        self.advance_char();
                        Symbol::AndAssign
                    }
                    _ => Symbol::BitAnd,
                }
            }
            '|' => {
                self.advance_char();
                match self.peek_char() {
                    Some('|') => {
                        self.advance_char();
                        Symbol::OrOr
                    }
                    Some('=') => {
                        self.advance_char();
                        Symbol::OrAssign
                    }
                    _ => Symbol::BitOr,
                }
            }
            '^' => {
                self.advance_char();
                if self.peek_char() == Some('=') {
                    self.advance_char();
                    Symbol::XorAssign
                } else {
                    Symbol::BitXor
                }
            }
            '~' => {
                self.advance_char();
                Symbol::BitNot
            }
            '?' => {
                self.advance_char();
                Symbol::Question
            }
            ':' => {
                self.advance_char();
                if self.peek_char() == Some(':') {
                    self.advance_char();
                    Symbol::ColonColon
                } else {
                    Symbol::Colon
                }
            }
            ',' => {
                self.advance_char();
                Symbol::Comma
            }
            ';' => {
                self.advance_char();
                Symbol::Semi
            }
            '(' => {
                self.advance_char();
                Symbol::LParen
            }
            ')' => {
                self.advance_char();
                Symbol::RParen
            }
            '{' => {
                self.advance_char();
                Symbol::LBrace
            }
            '}' => {
                self.advance_char();
                Symbol::RBrace
            }
            '[' => {
                self.advance_char();
                Symbol::LBracket
            }
            ']' => {
                self.advance_char();
                Symbol::RBracket
            }
            _ => {
                return Err(Error::lex(
                    format!("unexpected character {}", ch),
                    Some(location),
                ))
            }
        };
        Ok(symbol)
    }

    fn expect_char(&mut self, expected: char) -> Result<(), Error> {
        match self.peek_char() {
            Some(ch) if ch == expected => {
                self.advance_char();
                Ok(())
            }
            _ => Err(Error::lex(
                format!("expected '{}'", expected),
                Some(self.location()),
            )),
        }
    }

    fn peek_char(&self) -> Option<char> {
        self.chars.get(self.index).copied()
    }

    fn peek_next_char(&self) -> Option<char> {
        self.chars.get(self.index + 1).copied()
    }

    fn advance_char(&mut self) {
        if let Some(ch) = self.peek_char() {
            self.index += 1;
            if ch == '\n' {
                self.line += 1;
                self.column = 1;
            } else {
                self.column += 1;
            }
        }
    }

    fn is_ident_start(ch: char) -> bool {
        ch.is_ascii_alphabetic() || ch == '_'
    }

    fn is_ident_continue(ch: char) -> bool {
        ch.is_ascii_alphanumeric() || ch == '_'
    }
}
