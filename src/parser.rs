use crate::ast::{
    BinaryOp, Expr, Function, GlobalDecl, LocalDecl, Program, Stmt, UnaryOp,
};
use crate::error::{Error, SourceLocation};
use crate::lexer::{lex, Keyword, Symbol, Token, TokenKind};

pub fn parse_program(source: &str) -> Result<Program, Error> {
    let tokens = lex(source)?;
    let mut parser = Parser::new(tokens);
    parser.parse_program()
}

struct Parser {
    tokens: Vec<Token>,
    index: usize,
    current_locals: Vec<LocalDecl>,
}

impl Parser {
    fn new(tokens: Vec<Token>) -> Self {
        Self {
            tokens,
            index: 0,
            current_locals: Vec::new(),
        }
    }

    fn parse_program(&mut self) -> Result<Program, Error> {
        let mut includes = Vec::new();
        let mut globals = Vec::new();
        let mut functions = Vec::new();

        while !self.is_eof() {
            if self.match_keyword(Keyword::Include) {
                let name = self.expect_ident()?;
                includes.push(name);
                continue;
            }

            if self.match_keyword(Keyword::Extrn) {
                self.parse_extrn_decl(&mut globals)?;
                continue;
            }

            let name = self.expect_ident()?;
            if self.match_symbol(Symbol::LParen) {
                let func = self.parse_function(name)?;
                functions.push(func);
            } else {
                let decl = self.parse_global_decl(name)?;
                globals.push(decl);
            }
        }

        Ok(Program { includes, globals, functions })
    }

    fn parse_global_decl(&mut self, name: String) -> Result<GlobalDecl, Error> {
        let vector_size = if self.match_symbol(Symbol::LBracket) {
            let size = self.expect_number()? as usize;
            self.expect_symbol(Symbol::RBracket)?;
            Some(size)
        } else {
            None
        };
        self.expect_symbol(Symbol::Semi)?;
        Ok(GlobalDecl { name, vector_size })
    }

    fn parse_extrn_decl(&mut self, globals: &mut Vec<GlobalDecl>) -> Result<(), Error> {
        loop {
            let name = self.expect_ident()?;
            let vector_size = if self.match_symbol(Symbol::LBracket) {
                let size = self.expect_number()? as usize;
                self.expect_symbol(Symbol::RBracket)?;
                Some(size)
            } else {
                None
            };
            globals.push(GlobalDecl { name, vector_size });
            if self.match_symbol(Symbol::Comma) {
                continue;
            }
            self.expect_symbol(Symbol::Semi)?;
            break;
        }
        Ok(())
    }

    fn parse_function(&mut self, name: String) -> Result<Function, Error> {
        let mut params = Vec::new();
        if !self.match_symbol(Symbol::RParen) {
            loop {
                params.push(self.expect_ident()?);
                if self.match_symbol(Symbol::RParen) {
                    break;
                }
                self.expect_symbol(Symbol::Comma)?;
            }
        }

        let saved_locals = std::mem::take(&mut self.current_locals);
        let body = self.parse_compound()?;
        let locals = std::mem::take(&mut self.current_locals);
        self.current_locals = saved_locals;

        Ok(Function {
            name,
            params,
            locals,
            body,
        })
    }

    fn parse_compound(&mut self) -> Result<Stmt, Error> {
        self.expect_symbol(Symbol::LBrace)?;
        let mut stmts = Vec::new();
        while !self.match_symbol(Symbol::RBrace) {
            if self.check_keyword(Keyword::Auto) {
                self.parse_auto_decl()?;
                continue;
            }
            let stmt = self.parse_stmt()?;
            stmts.push(stmt);
        }
        Ok(Stmt::Compound(stmts))
    }

    fn parse_auto_decl(&mut self) -> Result<(), Error> {
        self.expect_keyword(Keyword::Auto)?;
        loop {
            let name = self.expect_ident()?;
            let vector_size = if self.match_symbol(Symbol::LBracket) {
                let size = self.expect_number()? as usize;
                self.expect_symbol(Symbol::RBracket)?;
                Some(size)
            } else {
                None
            };
            self.current_locals.push(LocalDecl { name, vector_size });
            if self.match_symbol(Symbol::Comma) {
                continue;
            }
            self.expect_symbol(Symbol::Semi)?;
            break;
        }
        Ok(())
    }

    fn parse_stmt(&mut self) -> Result<Stmt, Error> {
        if self.check_keyword(Keyword::Include) {
            return Err(self.error_here("'include' is not valid inside a function body"));
        }

        if self.match_symbol(Symbol::LBrace) {
            self.index -= 1;
            return self.parse_compound();
        }

        if self.match_keyword(Keyword::If) {
            self.expect_symbol(Symbol::LParen)?;
            let cond = self.parse_expr()?;
            self.expect_symbol(Symbol::RParen)?;
            let then_stmt = self.parse_stmt()?;
            let else_stmt = if self.match_keyword(Keyword::Else) {
                Some(Box::new(self.parse_stmt()?))
            } else {
                None
            };
            return Ok(Stmt::If(cond, Box::new(then_stmt), else_stmt));
        }

        if self.match_keyword(Keyword::While) {
            self.expect_symbol(Symbol::LParen)?;
            let cond = self.parse_expr()?;
            self.expect_symbol(Symbol::RParen)?;
            let body = self.parse_stmt()?;
            return Ok(Stmt::While(cond, Box::new(body)));
        }

        if self.match_keyword(Keyword::Switch) {
            self.expect_symbol(Symbol::LParen)?;
            let value = self.parse_expr()?;
            self.expect_symbol(Symbol::RParen)?;
            let body = self.parse_stmt()?;
            return Ok(Stmt::Switch(value, Box::new(body)));
        }

        if self.match_keyword(Keyword::Case) {
            let constant = self.parse_case_constant()?;
            self.expect_symbol(Symbol::Colon)?;
            let body = self.parse_stmt()?;
            return Ok(Stmt::Case(constant, Box::new(body)));
        }

        if self.match_keyword(Keyword::Default) {
            self.expect_symbol(Symbol::Colon)?;
            let body = self.parse_stmt()?;
            return Ok(Stmt::Default(Box::new(body)));
        }

        if self.match_keyword(Keyword::Break) {
            self.expect_symbol(Symbol::Semi)?;
            return Ok(Stmt::Break);
        }

        if self.match_keyword(Keyword::Return) {
            if self.match_symbol(Symbol::Semi) {
                return Ok(Stmt::Return(None));
            }
            let value = self.parse_expr()?;
            self.expect_symbol(Symbol::Semi)?;
            return Ok(Stmt::Return(Some(value)));
        }

        if self.match_keyword(Keyword::Goto) {
            let label = self.expect_ident()?;
            self.expect_symbol(Symbol::Semi)?;
            return Ok(Stmt::Goto(label));
        }

        if let Some((label, stmt)) = self.try_parse_label()? {
            return Ok(Stmt::Label(label, Box::new(stmt)));
        }

        let expr = self.parse_expr()?;
        self.expect_symbol(Symbol::Semi)?;
        Ok(Stmt::Expr(expr))
    }

    fn try_parse_label(&mut self) -> Result<Option<(String, Stmt)>, Error> {
        if let TokenKind::Ident(name) = self.peek().kind.clone() {
            if self.peek_next_symbol(Symbol::Colon) {
                self.advance();
                self.expect_symbol(Symbol::Colon)?;
                let stmt = self.parse_stmt()?;
                return Ok(Some((name, stmt)));
            }
        }
        Ok(None)
    }

    fn parse_case_constant(&mut self) -> Result<i64, Error> {
        match self.peek().kind.clone() {
            TokenKind::Number(value) => {
                self.advance();
                Ok(value)
            }
            TokenKind::CharConst(value) => {
                self.advance();
                Ok(value)
            }
            _ => Err(self.error_here("expected case constant")),
        }
    }

    fn parse_expr(&mut self) -> Result<Expr, Error> {
        self.parse_assignment()
    }

    fn parse_assignment(&mut self) -> Result<Expr, Error> {
        let mut left = self.parse_conditional()?;
        loop {
            let op = match self.peek().kind {
                TokenKind::Symbol(Symbol::Assign) => None,
                TokenKind::Symbol(Symbol::PlusAssign) => Some(BinaryOp::Add),
                TokenKind::Symbol(Symbol::MinusAssign) => Some(BinaryOp::Sub),
                TokenKind::Symbol(Symbol::StarAssign) => Some(BinaryOp::Mul),
                TokenKind::Symbol(Symbol::SlashAssign) => Some(BinaryOp::Div),
                TokenKind::Symbol(Symbol::PercentAssign) => Some(BinaryOp::Rem),
                TokenKind::Symbol(Symbol::LShiftAssign) => Some(BinaryOp::LShift),
                TokenKind::Symbol(Symbol::RShiftAssign) => Some(BinaryOp::RShift),
                TokenKind::Symbol(Symbol::AndAssign) => Some(BinaryOp::BitAnd),
                TokenKind::Symbol(Symbol::XorAssign) => Some(BinaryOp::BitXor),
                TokenKind::Symbol(Symbol::OrAssign) => Some(BinaryOp::BitOr),
                _ => return Ok(left),
            };

            self.advance();
            let right = self.parse_assignment()?;
            left = match op {
                Some(bin) => Expr::AssignOp(bin, Box::new(left), Box::new(right)),
                None => Expr::Assign(Box::new(left), Box::new(right)),
            };
        }
    }

    fn parse_conditional(&mut self) -> Result<Expr, Error> {
        let cond = self.parse_logical_or()?;
        if self.match_symbol(Symbol::Question) {
            let then_expr = self.parse_expr()?;
            self.expect_symbol(Symbol::Colon)?;
            let else_expr = self.parse_conditional()?;
            Ok(Expr::Conditional(
                Box::new(cond),
                Box::new(then_expr),
                Box::new(else_expr),
            ))
        } else {
            Ok(cond)
        }
    }

    fn parse_logical_or(&mut self) -> Result<Expr, Error> {
        self.parse_binary(Self::parse_logical_and, &[Symbol::OrOr], |_, _| {
            BinaryOp::Or
        })
    }

    fn parse_logical_and(&mut self) -> Result<Expr, Error> {
        self.parse_binary(Self::parse_bit_or, &[Symbol::AndAnd], |_, _| {
            BinaryOp::And
        })
    }

    fn parse_bit_or(&mut self) -> Result<Expr, Error> {
        self.parse_binary(Self::parse_bit_xor, &[Symbol::BitOr], |_, _| {
            BinaryOp::BitOr
        })
    }

    fn parse_bit_xor(&mut self) -> Result<Expr, Error> {
        self.parse_binary(Self::parse_bit_and, &[Symbol::BitXor], |_, _| {
            BinaryOp::BitXor
        })
    }

    fn parse_bit_and(&mut self) -> Result<Expr, Error> {
        self.parse_binary(Self::parse_equality, &[Symbol::BitAnd], |_, _| {
            BinaryOp::BitAnd
        })
    }

    fn parse_equality(&mut self) -> Result<Expr, Error> {
        self.parse_binary(
            Self::parse_relational,
            &[Symbol::Eq, Symbol::Ne],
            |symbol, _| match symbol {
                Symbol::Eq => BinaryOp::Eq,
                Symbol::Ne => BinaryOp::Ne,
                _ => BinaryOp::Eq,
            },
        )
    }

    fn parse_relational(&mut self) -> Result<Expr, Error> {
        self.parse_binary(
            Self::parse_shift,
            &[Symbol::Lt, Symbol::Le, Symbol::Gt, Symbol::Ge],
            |symbol, _| match symbol {
                Symbol::Lt => BinaryOp::Lt,
                Symbol::Le => BinaryOp::Le,
                Symbol::Gt => BinaryOp::Gt,
                Symbol::Ge => BinaryOp::Ge,
                _ => BinaryOp::Lt,
            },
        )
    }

    fn parse_shift(&mut self) -> Result<Expr, Error> {
        self.parse_binary(
            Self::parse_additive,
            &[Symbol::LShift, Symbol::RShift],
            |symbol, _| match symbol {
                Symbol::LShift => BinaryOp::LShift,
                Symbol::RShift => BinaryOp::RShift,
                _ => BinaryOp::LShift,
            },
        )
    }

    fn parse_additive(&mut self) -> Result<Expr, Error> {
        self.parse_binary(
            Self::parse_multiplicative,
            &[Symbol::Plus, Symbol::Minus],
            |symbol, _| match symbol {
                Symbol::Plus => BinaryOp::Add,
                Symbol::Minus => BinaryOp::Sub,
                _ => BinaryOp::Add,
            },
        )
    }

    fn parse_multiplicative(&mut self) -> Result<Expr, Error> {
        self.parse_binary(
            Self::parse_unary,
            &[Symbol::Star, Symbol::Slash, Symbol::Percent],
            |symbol, _| match symbol {
                Symbol::Star => BinaryOp::Mul,
                Symbol::Slash => BinaryOp::Div,
                Symbol::Percent => BinaryOp::Rem,
                _ => BinaryOp::Mul,
            },
        )
    }

    fn parse_unary(&mut self) -> Result<Expr, Error> {
        match self.peek().kind.clone() {
            TokenKind::Symbol(Symbol::Minus) => {
                self.advance();
                Ok(Expr::Unary(UnaryOp::Neg, Box::new(self.parse_unary()?)))
            }
            TokenKind::Symbol(Symbol::Not) => {
                self.advance();
                Ok(Expr::Unary(UnaryOp::Not, Box::new(self.parse_unary()?)))
            }
            TokenKind::Symbol(Symbol::BitNot) => {
                self.advance();
                Ok(Expr::Unary(UnaryOp::BitNot, Box::new(self.parse_unary()?)))
            }
            TokenKind::Symbol(Symbol::Star) => {
                self.advance();
                Ok(Expr::Indir(Box::new(self.parse_unary()?)))
            }
            TokenKind::Symbol(Symbol::BitAnd) => {
                self.advance();
                Ok(Expr::AddressOf(Box::new(self.parse_unary()?)))
            }
            TokenKind::Symbol(Symbol::PlusPlus) => {
                self.advance();
                Ok(Expr::Increment(Box::new(self.parse_unary()?), true))
            }
            TokenKind::Symbol(Symbol::MinusMinus) => {
                self.advance();
                Ok(Expr::Decrement(Box::new(self.parse_unary()?), true))
            }
            _ => self.parse_postfix(),
        }
    }

    fn parse_postfix(&mut self) -> Result<Expr, Error> {
        let mut expr = self.parse_primary()?;
        loop {
            if self.match_symbol(Symbol::LParen) {
                let mut args = Vec::new();
                if !self.match_symbol(Symbol::RParen) {
                    loop {
                        args.push(self.parse_expr()?);
                        if self.match_symbol(Symbol::RParen) {
                            break;
                        }
                        self.expect_symbol(Symbol::Comma)?;
                    }
                }
                expr = match expr {
                    Expr::Var(name) => Expr::Call(name, args),
                    _ => return Err(self.error_here("call target must be identifier")),
                };
                continue;
            }

            if self.match_symbol(Symbol::LBracket) {
                let index = self.parse_expr()?;
                self.expect_symbol(Symbol::RBracket)?;
                expr = Expr::Subscript(Box::new(expr), Box::new(index));
                continue;
            }

            if self.match_symbol(Symbol::PlusPlus) {
                expr = Expr::Increment(Box::new(expr), false);
                continue;
            }

            if self.match_symbol(Symbol::MinusMinus) {
                expr = Expr::Decrement(Box::new(expr), false);
                continue;
            }

            break;
        }
        Ok(expr)
    }

    fn parse_primary(&mut self) -> Result<Expr, Error> {
        match self.peek().kind.clone() {
            TokenKind::Number(value) => {
                self.advance();
                Ok(Expr::Constant(value))
            }
            TokenKind::CharConst(value) => {
                self.advance();
                Ok(Expr::CharConst(value))
            }
            TokenKind::StringLit(value) => {
                self.advance();
                Ok(Expr::StringLit(value))
            }
            TokenKind::Ident(name) => {
                self.advance();
                Ok(Expr::Var(name))
            }
            TokenKind::Symbol(Symbol::LParen) => {
                self.advance();
                let expr = self.parse_expr()?;
                self.expect_symbol(Symbol::RParen)?;
                Ok(expr)
            }
            _ => Err(self.error_here("unexpected token in expression")),
        }
    }

    fn parse_binary<F>(&mut self, next: F, symbols: &[Symbol], map: fn(Symbol, SourceLocation) -> BinaryOp) -> Result<Expr, Error>
    where
        F: Fn(&mut Self) -> Result<Expr, Error>,
    {
        let mut expr = next(self)?;
        loop {
            let symbol = match self.peek().kind.clone() {
                TokenKind::Symbol(sym) if symbols.contains(&sym) => sym,
                _ => break,
            };
            let location = self.peek().location.clone();
            self.advance();
            let rhs = next(self)?;
            let op = map(symbol, location);
            expr = Expr::Binary(op, Box::new(expr), Box::new(rhs));
        }
        Ok(expr)
    }

    fn expect_symbol(&mut self, symbol: Symbol) -> Result<(), Error> {
        if self.match_symbol(symbol.clone()) {
            Ok(())
        } else {
            Err(self.error_here(format!("expected {:?}", symbol)))
        }
    }

    fn match_symbol(&mut self, symbol: Symbol) -> bool {
        if let TokenKind::Symbol(sym) = self.peek().kind.clone() {
            if sym == symbol {
                self.advance();
                return true;
            }
        }
        false
    }

    fn peek_next_symbol(&self, symbol: Symbol) -> bool {
        if let Some(token) = self.tokens.get(self.index + 1) {
            if let TokenKind::Symbol(sym) = &token.kind {
                return sym == &symbol;
            }
        }
        false
    }

    fn expect_keyword(&mut self, keyword: Keyword) -> Result<(), Error> {
        if self.match_keyword(keyword.clone()) {
            Ok(())
        } else {
            Err(self.error_here(format!("expected {:?}", keyword)))
        }
    }

    fn match_keyword(&mut self, keyword: Keyword) -> bool {
        if let TokenKind::Keyword(key) = self.peek().kind.clone() {
            if key == keyword {
                self.advance();
                return true;
            }
        }
        false
    }

    fn check_keyword(&self, keyword: Keyword) -> bool {
        matches!(&self.peek().kind, TokenKind::Keyword(key) if key == &keyword)
    }

    fn expect_ident(&mut self) -> Result<String, Error> {
        match self.peek().kind.clone() {
            TokenKind::Ident(name) => {
                self.advance();
                Ok(name)
            }
            _ => Err(self.error_here("expected identifier")),
        }
    }

    fn expect_number(&mut self) -> Result<i64, Error> {
        match self.peek().kind.clone() {
            TokenKind::Number(value) => {
                self.advance();
                Ok(value)
            }
            _ => Err(self.error_here("expected number")),
        }
    }

    fn peek(&self) -> &Token {
        self.tokens
            .get(self.index)
            .unwrap_or_else(|| self.tokens.last().expect("token list"))
    }

    fn advance(&mut self) {
        if self.index < self.tokens.len() {
            self.index += 1;
        }
    }

    fn is_eof(&self) -> bool {
        matches!(self.peek().kind, TokenKind::Eof)
    }

    fn error_here(&self, message: impl Into<String>) -> Error {
        let location = self.peek().location.clone();
        Error::parse(message, Some(location))
    }
}
