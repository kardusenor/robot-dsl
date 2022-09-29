use crate::error::{parser_error, Error};
use crate::syntax::{Expr, LiteralValue, Stmt};
use crate::token::{Token, TokenType};

pub struct Parser<'t> {
    pub tokens: &'t Vec<Token>,
    current: usize,
}

macro_rules! matches {
    ( $sel:ident, $( $x:expr ),* ) => {
        {
            if $( $sel.check($x) )||* {
                $sel.advance();
                true
            } else {
                false
            }
        }
    };
}

impl<'t> Parser<'t> {
    pub fn new(tokens: &'t Vec<Token>) -> Self {
        Parser { tokens, current: 0 }
    }

    // for test expression calculating
    pub fn calculate(&mut self) -> Option<Expr> {
        self.expression().ok()
    }

    pub fn parse(&mut self) -> Result<Vec<Stmt>, Error> {
        let mut statements: Vec<Stmt> = Vec::new();
        while !self.is_at_end() {
            statements.push(self.declaration()?);
        }
        Ok(statements)
    }

    fn declaration(&mut self) -> Result<Stmt, Error> {
        let statement = if matches!(self, TokenType::Var) {
            self.var_declaration()
        } else {
            self.statement()
        };

        match statement {
            Err(Error::Parse) => {
                self.synchronize();
                Ok(Stmt::Null)
            }
            other => other,
        }
    }

    fn statement(&mut self) -> Result<Stmt, Error> {
        if matches!(self, TokenType::Speak) {
            self.speak_statement()
        } else if matches!(self, TokenType::LeftBrace) {
            Ok(Stmt::Block {
                statements: self.block()?,
            })
        } else {
            self.expression_statement()
        }
    }

    fn speak_statement(&mut self) -> Result<Stmt, Error> {
        let value = self.expression()?;
        self.consume(TokenType::SemiColon, "Expect ';' after value.")?;
        Ok(Stmt::Speak { expression: value })
    }

    fn var_declaration(&mut self) -> Result<Stmt, Error> {
        let name = self.consume(TokenType::Identifier, "Expect variable name.")?;

        let initializer = if matches!(self, TokenType::Equal) {
            Some(self.expression()?)
        } else {
            None
        };

        self.consume(
            TokenType::SemiColon,
            "Expect ';' after variable declaration.",
        )?;
        Ok(Stmt::Var { name, initializer })
    }

    fn expression_statement(&mut self) -> Result<Stmt, Error> {
        let expr = self.expression()?;
        self.consume(TokenType::SemiColon, "Expect ';' after expression.")?;
        Ok(Stmt::Expression { expression: expr })
    }

    fn block(&mut self) -> Result<Vec<Stmt>, Error> {
        let mut statements: Vec<Stmt> = Vec::new();

        while !self.check(TokenType::RightBrace) && !self.is_at_end() {
            statements.push(self.declaration()?);
        }

        self.consume(TokenType::RightBrace, "Expect '}' after block.")?;
        Ok(statements)
    }


    pub fn expression(&mut self) -> Result<Expr, Error> {
        self.assignment()
    }

    fn equality(&mut self) -> Result<Expr, Error> {
        let mut expr = self.addition()?;

        while matches!(self, TokenType::BangEqual, TokenType::EqualEqual) {
            let operator = self.previous().clone();
            let right = self.addition()?;
            expr = Expr::Binary {
                left: Box::new(expr),
                operator,
                right: Box::new(right),
            };
        }

        Ok(expr)
    }

    fn assignment(&mut self) -> Result<Expr, Error> {
        let expr = self.equality()?;

        if matches!(self, TokenType::Equal) {
            let value = Box::new(self.assignment()?);

            if let Expr::Variable { name } = expr {
                return Ok(Expr::Assign { name, value });
            }

            let equals = self.previous();
            self.error(equals, "Invalid assignment target.");
        }

        Ok(expr)
    }

    fn addition(&mut self) -> Result<Expr, Error> {
        let mut expr = self.unary()?;

        while matches!(self, TokenType::Plus, TokenType::Minus) {
            let operator = self.previous().clone();
            let right = self.unary()?;
            expr = Expr::Binary {
                left: Box::new(expr),
                operator,
                right: Box::new(right),
            };
        }

        Ok(expr)
    }

    fn unary(&mut self) -> Result<Expr, Error> {
        if matches!(self, TokenType::Bang) {
            let operator = self.previous().clone();
            let right = self.unary()?;
            Ok(Expr::Unary {
                operator,
                right: Box::new(right),
            })
        } else {
            self.primary()
        }
    }

    fn primary(&mut self) -> Result<Expr, Error> {
        let expr = match &self.peek().tpe {
            TokenType::False => Expr::Literal {
                value: LiteralValue::Boolean(false),
            },
            TokenType::True => Expr::Literal {
                value: LiteralValue::Boolean(true),
            },
            /*
            TokenType::Nil => Expr::Literal {
                value: LiteralValue::Nil,
            },
            */
            TokenType::Number { literal } => Expr::Literal {
                value: LiteralValue::Number(literal.clone()),
            },
            TokenType::String { literal } => Expr::Literal {
                value: LiteralValue::String(literal.clone()),
            },
            TokenType::Identifier => Expr::Variable {
                name: self.peek().clone(),
            },
            _ => return Err(self.error(self.peek(), "Expected expression")),
        };

        self.advance();

        Ok(expr)
    }

    fn check(&self, token_type: TokenType) -> bool {
        if self.is_at_end() {
            return false;
        }

        token_type == self.peek().tpe
    }

    fn is_at_end(&self) -> bool {
        self.peek().tpe == TokenType::EOF
    }

    fn error(&self, token: &Token, message: &str) -> Error {
        parser_error(token, message);
        Error::Parse
    }

    fn advance(&mut self) -> &Token {
        if !self.is_at_end() {
            self.current += 1;
        }

        self.previous()
    }

    fn previous(&self) -> &Token {
        self.tokens
            .get(self.current - 1)
            .expect("No previous token")
    }

    fn peek(&self) -> &Token {
        self.tokens
            .get(self.current)
            .expect("Peek into end of token stream.")
    }

    fn consume(&mut self, token_type: TokenType, message: &str) -> Result<Token, Error> {
        if self.check(token_type) {
            Ok(self.advance().clone())
        } else {
            Err(self.error(self.peek(), message))
        }
    }

    fn synchronize(&mut self) {
        self.advance();

        while !self.is_at_end() {
            if self.previous().tpe == TokenType::SemiColon {
                return;
            }

            match self.peek().tpe {
                TokenType::Var
                | TokenType::Branch
                | TokenType::Exit
                | TokenType::Input
                | TokenType::Listen
                | TokenType::Speak
                | TokenType::Step => return,
                _ => self.advance(),
            };
        }
    }
}
