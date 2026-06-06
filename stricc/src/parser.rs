use crate::ast::*;
use crate::error::{Span, Diagnostic};
use crate::lexer::{Lexer, Token, TokenKind};

pub struct Parser<'a> {
    tokens: Vec<Token>,
    position: usize,
    pub errors: Vec<Diagnostic>,
    filename: String,
    source_code: &'a str,
}

impl<'a> Parser<'a> {
    pub fn new(source: &'a str, filename: &str) -> Self {
        let mut lexer = Lexer::new(source);
        let mut tokens = Vec::new();
        let mut errors = Vec::new();

        let mut current_filename = filename.to_string();
        loop {
            match lexer.next_token() {
                Ok(token) => {
                    match &token.kind {
                        TokenKind::LineMarker { line: _, filename: fm } => {
                            current_filename = fm.clone();
                        }
                        TokenKind::EOF => {
                            tokens.push(token);
                            break;
                        }
                        _ => {
                            tokens.push(token);
                        }
                    }
                }
                Err(err) => {
                    errors.push(Diagnostic::error(
                        format!("Lexer error: {}", err),
                        &current_filename,
                    ));
                    break;
                }
            }
        }

        Self {
            tokens,
            position: 0,
            errors,
            filename: filename.to_string(),
            source_code: source,
        }
    }

    fn current_token(&self) -> &Token {
        &self.tokens[self.position]
    }

    fn advance(&mut self) -> &Token {
        if !self.is_at_end() {
            self.position += 1;
        }
        &self.tokens[self.position - 1]
    }

    fn is_at_end(&self) -> bool {
        matches!(self.current_token().kind, TokenKind::EOF)
    }

    fn check(&self, kind: &TokenKind) -> bool {
        if self.is_at_end() {
            return false;
        }
        std::mem::discriminant(&self.current_token().kind) == std::mem::discriminant(kind)
    }

    fn match_token(&mut self, kind: &TokenKind) -> bool {
        if self.check(kind) {
            self.advance();
            true
        } else {
            false
        }
    }

    fn consume(&mut self, kind: &TokenKind, message: &str) -> Result<Token, String> {
        if self.check(kind) {
            Ok(self.advance().clone())
        } else {
            let token = self.current_token();
            let err = Diagnostic::error_with_span(
                format!("Expected {:?}, found {:?}", kind, token.kind),
                token.span,
                message.to_string(),
                &self.filename,
            );
            Err(format!("Parser error: {}", message))
        }
    }

    // Error recovery: synchronize parser state to next statement boundary
    fn synchronize(&mut self) {
        self.advance();
        while !self.is_at_end() {
            if matches!(self.tokens[self.position - 1].kind, TokenKind::Semicolon) {
                return;
            }
            match self.current_token().kind {
                TokenKind::Struct
                | TokenKind::Union
                | TokenKind::Enum
                | TokenKind::If
                | TokenKind::While
                | TokenKind::For
                | TokenKind::Switch
                | TokenKind::Return
                | TokenKind::Unsafe
                | TokenKind::LBrace
                | TokenKind::RBrace => {
                    return;
                }
                _ => {}
            }
            self.advance();
        }
    }

    pub fn parse_program(&mut self) -> Program {
        let mut decls = Vec::new();
        while !self.is_at_end() {
            match self.parse_global_decl() {
                Ok(decl) => decls.push(decl),
                Err(err) => {
                    self.errors.push(Diagnostic::error_with_span(
                        err.clone(),
                        self.current_token().span,
                        "Failed to parse global declaration".to_string(),
                        &self.filename,
                    ));
                    self.synchronize();
                }
            }
        }
        Program { decls }
    }

    fn parse_global_decl(&mut self) -> Result<GlobalDecl, String> {
        let start_span = self.current_token().span;
        
        // Check struct definition
        if self.match_token(&TokenKind::Struct) {
            let name_tok = self.consume(&TokenKind::Identifier(String::new()), "Expected struct name")?;
            let name = match name_tok.kind {
                TokenKind::Identifier(n) => n,
                _ => unreachable!(),
            };

            if self.match_token(&TokenKind::LBrace) {
                let mut fields = Vec::new();
                while !self.check(&TokenKind::RBrace) && !self.is_at_end() {
                    let field_ty = self.parse_type()?;
                    let field_name_tok = self.consume(&TokenKind::Identifier(String::new()), "Expected field name")?;
                    let field_name = match field_name_tok.kind {
                        TokenKind::Identifier(n) => n,
                        _ => unreachable!(),
                    };
                    self.consume(&TokenKind::Semicolon, "Expected ';' after field declaration")?;
                    fields.push(Field { name: field_name, ty: field_ty });
                }
                self.consume(&TokenKind::RBrace, "Expected '}' to close struct definition")?;
                self.consume(&TokenKind::Semicolon, "Expected ';' after struct declaration")?;
                let end_span = self.tokens[self.position - 1].span;
                return Ok(GlobalDecl::Struct(StructDecl {
                    name,
                    fields,
                    span: start_span.union(end_span),
                }));
            } else {
                // Struct tag used in variable or function declaration
                self.position -= 2; // backtrack Struct name_tok
            }
        }

        // Parse global variable or function declaration
        let base_ty = self.parse_type()?;
        let name_tok = self.consume(&TokenKind::Identifier(String::new()), "Expected name")?;
        let name = match name_tok.kind {
            TokenKind::Identifier(n) => n,
            _ => unreachable!(),
        };

        if self.match_token(&TokenKind::LParen) {
            // Function declaration or definition
            let mut params = Vec::new();
            let mut is_variadic = false;
            if !self.check(&TokenKind::RParen) {
                loop {
                    // Check for ellipsis
                    if self.match_token(&TokenKind::Dot) {
                        self.consume(&TokenKind::Dot, "Expected '...'")?;
                        self.consume(&TokenKind::Dot, "Expected '...'")?;
                        is_variadic = true;
                        break;
                    }
                    let param_ty = self.parse_type()?;
                    let param_name_tok = self.consume(&TokenKind::Identifier(String::new()), "Expected parameter name")?;
                    let param_name = match param_name_tok.kind {
                        TokenKind::Identifier(n) => n,
                        _ => unreachable!(),
                    };
                    params.push(Param { name: param_name, ty: param_ty });
                    if !self.match_token(&TokenKind::Comma) {
                        break;
                    }
                }
            }
            self.consume(&TokenKind::RParen, "Expected ')' after parameters")?;

            if self.match_token(&TokenKind::LBrace) {
                // Function definition
                self.position -= 1; // back to LBrace
                let body = self.parse_stmt()?;
                let end_span = self.tokens[self.position - 1].span;
                Ok(GlobalDecl::Function(FunctionDecl {
                    name,
                    params,
                    return_type: base_ty,
                    is_variadic,
                    body: Some(body),
                    span: start_span.union(end_span),
                }))
            } else {
                // Prototype
                self.consume(&TokenKind::Semicolon, "Expected ';' after prototype")?;
                let end_span = self.tokens[self.position - 1].span;
                Ok(GlobalDecl::Function(FunctionDecl {
                    name,
                    params,
                    return_type: base_ty,
                    is_variadic,
                    body: None,
                    span: start_span.union(end_span),
                }))
            }
        } else {
            // Global variable declaration
            let mut init = None;
            if self.match_token(&TokenKind::Equal) {
                init = Some(self.parse_expr()?);
            }
            self.consume(&TokenKind::Semicolon, "Expected ';' after variable declaration")?;
            let end_span = self.tokens[self.position - 1].span;
            Ok(GlobalDecl::GlobalVar(base_ty, name, init, start_span.union(end_span)))
        }
    }

    fn parse_type(&mut self) -> Result<Type, String> {
        let mut ty = if self.match_token(&TokenKind::Const) {
            // Const qualifier - in this subset, we'll wrap it or parse it natively
            self.parse_base_type()?
        } else {
            self.parse_base_type()?
        };

        // Pointer qualification
        while self.match_token(&TokenKind::Star) {
            ty = Type::Pointer(Box::new(ty));
        }

        // Array qualification
        if self.match_token(&TokenKind::LBracket) {
            let len_tok = self.consume(&TokenKind::IntLiteral(0), "Expected array length")?;
            let len = match len_tok.kind {
                TokenKind::IntLiteral(v) => v as usize,
                _ => unreachable!(),
            };
            self.consume(&TokenKind::RBracket, "Expected ']'")?;
            ty = Type::Array(Box::new(ty), len);
        }

        Ok(ty)
    }

    fn parse_base_type(&mut self) -> Result<Type, String> {
        if self.match_token(&TokenKind::Int) {
            Ok(Type::Int)
        } else if self.match_token(&TokenKind::Char) {
            Ok(Type::Char)
        } else if self.match_token(&TokenKind::Float) {
            Ok(Type::Float)
        } else if self.match_token(&TokenKind::Double) {
            Ok(Type::Double)
        } else if self.match_token(&TokenKind::Short) {
            Ok(Type::Short)
        } else if self.match_token(&TokenKind::Long) {
            Ok(Type::Long)
        } else if self.match_token(&TokenKind::Bool) {
            Ok(Type::Bool)
        } else if self.match_token(&TokenKind::Void) {
            Ok(Type::Void)
        } else if self.match_token(&TokenKind::Auto) {
            Ok(Type::Auto)
        } else if self.match_token(&TokenKind::Nullptr) {
            Ok(Type::Nullptr)
        } else if self.match_token(&TokenKind::Unsigned) {
            if self.match_token(&TokenKind::Int) {
                Ok(Type::UnsignedInt)
            } else if self.match_token(&TokenKind::Char) {
                Ok(Type::UnsignedChar)
            } else if self.match_token(&TokenKind::Short) {
                Ok(Type::UnsignedShort)
            } else if self.match_token(&TokenKind::Long) {
                Ok(Type::UnsignedLong)
            } else {
                Ok(Type::UnsignedInt)
            }
        } else if self.match_token(&TokenKind::Signed) {
            if self.match_token(&TokenKind::Int) {
                Ok(Type::Int)
            } else if self.match_token(&TokenKind::Char) {
                Ok(Type::Char)
            } else {
                Ok(Type::Int)
            }
        } else if self.match_token(&TokenKind::Struct) {
            let name_tok = self.consume(&TokenKind::Identifier(String::new()), "Expected struct name")?;
            match name_tok.kind {
                TokenKind::Identifier(n) => Ok(Type::Struct(n)),
                _ => unreachable!(),
            }
        } else if self.match_token(&TokenKind::Union) {
            let name_tok = self.consume(&TokenKind::Identifier(String::new()), "Expected union name")?;
            match name_tok.kind {
                TokenKind::Identifier(n) => Ok(Type::Union(n)),
                _ => unreachable!(),
            }
        } else if self.match_token(&TokenKind::Enum) {
            let name_tok = self.consume(&TokenKind::Identifier(String::new()), "Expected enum name")?;
            match name_tok.kind {
                TokenKind::Identifier(n) => Ok(Type::Enum(n)),
                _ => unreachable!(),
            }
        } else if self.match_token(&TokenKind::Atomic) {
            self.consume(&TokenKind::LParen, "Expected '(' after _Atomic")?;
            let inner_ty = self.parse_type()?;
            self.consume(&TokenKind::RParen, "Expected ')'")?;
            Ok(Type::Atomic(Box::new(inner_ty)))
        } else if self.match_token(&TokenKind::Typeof) {
            self.consume(&TokenKind::LParen, "Expected '(' after typeof")?;
            // Determine if expression or type. Let's see: if first token is a type keyword, parse type.
            let is_type = self.check(&TokenKind::Int)
                || self.check(&TokenKind::Char)
                || self.check(&TokenKind::Float)
                || self.check(&TokenKind::Double)
                || self.check(&TokenKind::Void)
                || self.check(&TokenKind::Struct)
                || self.check(&TokenKind::Union);
            let res = if is_type {
                let inner_ty = self.parse_type()?;
                Type::TypeofType(Box::new(inner_ty))
            } else {
                let inner_expr = self.parse_expr()?;
                Type::TypeofExpression(Box::new(inner_expr))
            };
            self.consume(&TokenKind::RParen, "Expected ')'")?;
            Ok(res)
        } else {
            Err(format!(
                "Unknown type identifier: {:?}",
                self.current_token().kind
            ))
        }
    }

    fn parse_stmt(&mut self) -> Result<Stmt, String> {
        let start_span = self.current_token().span;

        if self.match_token(&TokenKind::LBrace) {
            let mut stmts = Vec::new();
            while !self.check(&TokenKind::RBrace) && !self.is_at_end() {
                match self.parse_stmt() {
                    Ok(s) => stmts.push(s),
                    Err(e) => {
                        self.errors.push(Diagnostic::error_with_span(
                            e,
                            self.current_token().span,
                            "Failed to parse statement in block".to_string(),
                            &self.filename,
                        ));
                        self.synchronize();
                    }
                }
            }
            self.consume(&TokenKind::RBrace, "Expected '}'")?;
            let end_span = self.tokens[self.position - 1].span;
            Ok(Stmt {
                node: StmtNode::Compound(stmts),
                span: start_span.union(end_span),
            })
        } else if self.match_token(&TokenKind::Unsafe) {
            // __unsafe block statement
            let inner_block = self.parse_stmt()?;
            let end_span = self.tokens[self.position - 1].span;
            Ok(Stmt {
                node: StmtNode::Unsafe(Box::new(inner_block)),
                span: start_span.union(end_span),
            })
        } else if self.match_token(&TokenKind::If) {
            self.consume(&TokenKind::LParen, "Expected '(' after if")?;
            let cond = self.parse_expr()?;
            self.consume(&TokenKind::RParen, "Expected ')' after if condition")?;
            let then_branch = self.parse_stmt()?;
            let mut else_branch = None;
            if self.match_token(&TokenKind::Else) {
                else_branch = Some(Box::new(self.parse_stmt()?));
            }
            let end_span = self.tokens[self.position - 1].span;
            Ok(Stmt {
                node: StmtNode::If(cond, Box::new(then_branch), else_branch),
                span: start_span.union(end_span),
            })
        } else if self.match_token(&TokenKind::While) {
            self.consume(&TokenKind::LParen, "Expected '(' after while")?;
            let cond = self.parse_expr()?;
            self.consume(&TokenKind::RParen, "Expected ')' after while condition")?;
            let body = self.parse_stmt()?;
            let end_span = self.tokens[self.position - 1].span;
            Ok(Stmt {
                node: StmtNode::While(cond, Box::new(body)),
                span: start_span.union(end_span),
            })
        } else if self.match_token(&TokenKind::For) {
            self.consume(&TokenKind::LParen, "Expected '(' after for")?;
            
            let mut init = None;
            if !self.match_token(&TokenKind::Semicolon) {
                init = Some(Box::new(self.parse_stmt()?)); // Handles decl or expr-stmt (which has semicolon)
            }

            let mut cond = None;
            if !self.match_token(&TokenKind::Semicolon) {
                cond = Some(self.parse_expr()?);
                self.consume(&TokenKind::Semicolon, "Expected ';' after for condition")?;
            }

            let mut post = None;
            if !self.check(&TokenKind::RParen) {
                post = Some(self.parse_expr()?);
            }
            self.consume(&TokenKind::RParen, "Expected ')' after for parameters")?;
            
            let body = self.parse_stmt()?;
            let end_span = self.tokens[self.position - 1].span;
            Ok(Stmt {
                node: StmtNode::For(init, cond, post, Box::new(body)),
                span: start_span.union(end_span),
            })
        } else if self.match_token(&TokenKind::Switch) {
            self.consume(&TokenKind::LParen, "Expected '(' after switch")?;
            let cond = self.parse_expr()?;
            self.consume(&TokenKind::RParen, "Expected ')' after switch expression")?;
            let body = self.parse_stmt()?;
            let end_span = self.tokens[self.position - 1].span;
            Ok(Stmt {
                node: StmtNode::Switch(cond, Box::new(body)),
                span: start_span.union(end_span),
            })
        } else if self.match_token(&TokenKind::Case) {
            let val = self.parse_expr()?;
            self.consume(&TokenKind::Colon, "Expected ':' after case value")?;
            let body = self.parse_stmt()?;
            let end_span = self.tokens[self.position - 1].span;
            Ok(Stmt {
                node: StmtNode::Case(val, Box::new(body)),
                span: start_span.union(end_span),
            })
        } else if self.match_token(&TokenKind::Default) {
            self.consume(&TokenKind::Colon, "Expected ':' after default")?;
            let body = self.parse_stmt()?;
            let end_span = self.tokens[self.position - 1].span;
            Ok(Stmt {
                node: StmtNode::Default(Box::new(body)),
                span: start_span.union(end_span),
            })
        } else if self.match_token(&TokenKind::Break) {
            self.consume(&TokenKind::Semicolon, "Expected ';'")?;
            let end_span = self.tokens[self.position - 1].span;
            Ok(Stmt {
                node: StmtNode::Break,
                span: start_span.union(end_span),
            })
        } else if self.match_token(&TokenKind::Continue) {
            self.consume(&TokenKind::Semicolon, "Expected ';'")?;
            let end_span = self.tokens[self.position - 1].span;
            Ok(Stmt {
                node: StmtNode::Continue,
                span: start_span.union(end_span),
            })
        } else if self.match_token(&TokenKind::Return) {
            let mut val = None;
            if !self.check(&TokenKind::Semicolon) {
                val = Some(self.parse_expr()?);
            }
            self.consume(&TokenKind::Semicolon, "Expected ';' after return")?;
            let end_span = self.tokens[self.position - 1].span;
            Ok(Stmt {
                node: StmtNode::Return(val),
                span: start_span.union(end_span),
            })
        } else {
            // Check if variable declaration (e.g. starts with type keyword or typeof/const/auto)
            let is_type = self.check(&TokenKind::Int)
                || self.check(&TokenKind::Char)
                || self.check(&TokenKind::Float)
                || self.check(&TokenKind::Double)
                || self.check(&TokenKind::Short)
                || self.check(&TokenKind::Long)
                || self.check(&TokenKind::Bool)
                || self.check(&TokenKind::Void)
                || self.check(&TokenKind::Struct)
                || self.check(&TokenKind::Union)
                || self.check(&TokenKind::Enum)
                || self.check(&TokenKind::Auto)
                || self.check(&TokenKind::Const)
                || self.check(&TokenKind::Typeof)
                || self.check(&TokenKind::Atomic);

            if is_type {
                let ty = self.parse_type()?;
                let name_tok = self.consume(&TokenKind::Identifier(String::new()), "Expected variable name")?;
                let name = match name_tok.kind {
                    TokenKind::Identifier(n) => n,
                    _ => unreachable!(),
                };

                let mut init = None;
                if self.match_token(&TokenKind::Equal) {
                    init = Some(self.parse_expr()?);
                }
                self.consume(&TokenKind::Semicolon, "Expected ';' after declaration")?;
                let end_span = self.tokens[self.position - 1].span;
                Ok(Stmt {
                    node: StmtNode::Decl(ty, name, init),
                    span: start_span.union(end_span),
                })
            } else {
                // Expression statement
                let expr = self.parse_expr()?;
                self.consume(&TokenKind::Semicolon, "Expected ';' after expression")?;
                let end_span = self.tokens[self.position - 1].span;
                Ok(Stmt {
                    node: StmtNode::Expr(expr),
                    span: start_span.union(end_span),
                })
            }
        }
    }

    fn parse_expr(&mut self) -> Result<Expr, String> {
        self.parse_assignment()
    }

    fn parse_assignment(&mut self) -> Result<Expr, String> {
        let left = self.parse_logical_or()?;
        if self.match_token(&TokenKind::Equal) {
            let right = self.parse_assignment()?;
            let span = left.span.union(right.span);
            return Ok(Expr {
                node: ExprNode::Assign(Box::new(left), Box::new(right)),
                span,
                ty: None,
            });
        }
        Ok(left)
    }

    fn parse_logical_or(&mut self) -> Result<Expr, String> {
        let mut left = self.parse_logical_and()?;
        while self.match_token(&TokenKind::PipePipe) {
            let right = self.parse_logical_and()?;
            let span = left.span.union(right.span);
            left = Expr {
                node: ExprNode::Binary(BinaryOp::LogicalOr, Box::new(left), Box::new(right)),
                span,
                ty: None,
            };
        }
        Ok(left)
    }

    fn parse_logical_and(&mut self) -> Result<Expr, String> {
        let mut left = self.parse_bitwise_or()?;
        while self.match_token(&TokenKind::AmpAmp) {
            let right = self.parse_bitwise_or()?;
            let span = left.span.union(right.span);
            left = Expr {
                node: ExprNode::Binary(BinaryOp::LogicalAnd, Box::new(left), Box::new(right)),
                span,
                ty: None,
            };
        }
        Ok(left)
    }

    fn parse_bitwise_or(&mut self) -> Result<Expr, String> {
        let mut left = self.parse_bitwise_xor()?;
        while self.match_token(&TokenKind::Pipe) {
            let right = self.parse_bitwise_xor()?;
            let span = left.span.union(right.span);
            left = Expr {
                node: ExprNode::Binary(BinaryOp::BitOr, Box::new(left), Box::new(right)),
                span,
                ty: None,
            };
        }
        Ok(left)
    }

    fn parse_bitwise_xor(&mut self) -> Result<Expr, String> {
        let mut left = self.parse_bitwise_and()?;
        while self.match_token(&TokenKind::Caret) {
            let right = self.parse_bitwise_and()?;
            let span = left.span.union(right.span);
            left = Expr {
                node: ExprNode::Binary(BinaryOp::BitXor, Box::new(left), Box::new(right)),
                span,
                ty: None,
            };
        }
        Ok(left)
    }

    fn parse_bitwise_and(&mut self) -> Result<Expr, String> {
        let mut left = self.parse_equality()?;
        while self.match_token(&TokenKind::Ampersand) {
            let right = self.parse_equality()?;
            let span = left.span.union(right.span);
            left = Expr {
                node: ExprNode::Binary(BinaryOp::BitAnd, Box::new(left), Box::new(right)),
                span,
                ty: None,
            };
        }
        Ok(left)
    }

    fn parse_equality(&mut self) -> Result<Expr, String> {
        let mut left = self.parse_relational()?;
        while self.check(&TokenKind::EqualEqual) || self.check(&TokenKind::BangEqual) {
            let op = if self.match_token(&TokenKind::EqualEqual) {
                BinaryOp::Equal
            } else {
                self.advance();
                BinaryOp::NotEqual
            };
            let right = self.parse_relational()?;
            let span = left.span.union(right.span);
            left = Expr {
                node: ExprNode::Binary(op, Box::new(left), Box::new(right)),
                span,
                ty: None,
            };
        }
        Ok(left)
    }

    fn parse_relational(&mut self) -> Result<Expr, String> {
        let mut left = self.parse_shift()?;
        while self.check(&TokenKind::Less)
            || self.check(&TokenKind::LessEqual)
            || self.check(&TokenKind::Greater)
            || self.check(&TokenKind::GreaterEqual)
        {
            let op = if self.match_token(&TokenKind::Less) {
                BinaryOp::Less
            } else if self.match_token(&TokenKind::LessEqual) {
                BinaryOp::LessEqual
            } else if self.match_token(&TokenKind::Greater) {
                BinaryOp::Greater
            } else {
                self.advance();
                BinaryOp::GreaterEqual
            };
            let right = self.parse_shift()?;
            let span = left.span.union(right.span);
            left = Expr {
                node: ExprNode::Binary(op, Box::new(left), Box::new(right)),
                span,
                ty: None,
            };
        }
        Ok(left)
    }

    fn parse_shift(&mut self) -> Result<Expr, String> {
        let mut left = self.parse_additive()?;
        while self.check(&TokenKind::LessLess) || self.check(&TokenKind::GreaterGreater) {
            let op = if self.match_token(&TokenKind::LessLess) {
                BinaryOp::Shl
            } else {
                self.advance();
                BinaryOp::Shr
            };
            let right = self.parse_additive()?;
            let span = left.span.union(right.span);
            left = Expr {
                node: ExprNode::Binary(op, Box::new(left), Box::new(right)),
                span,
                ty: None,
            };
        }
        Ok(left)
    }

    fn parse_additive(&mut self) -> Result<Expr, String> {
        let mut left = self.parse_multiplicative()?;
        while self.check(&TokenKind::Plus) || self.check(&TokenKind::Minus) {
            let op = if self.match_token(&TokenKind::Plus) {
                BinaryOp::Add
            } else {
                self.advance();
                BinaryOp::Sub
            };
            let right = self.parse_multiplicative()?;
            let span = left.span.union(right.span);
            left = Expr {
                node: ExprNode::Binary(op, Box::new(left), Box::new(right)),
                span,
                ty: None,
            };
        }
        Ok(left)
    }

    fn parse_multiplicative(&mut self) -> Result<Expr, String> {
        let mut left = self.parse_unary()?;
        while self.check(&TokenKind::Star) || self.check(&TokenKind::Slash) || self.check(&TokenKind::Percent) {
            let op = if self.match_token(&TokenKind::Star) {
                BinaryOp::Mul
            } else if self.match_token(&TokenKind::Slash) {
                BinaryOp::Div
            } else {
                self.advance();
                BinaryOp::Mod
            };
            let right = self.parse_unary()?;
            let span = left.span.union(right.span);
            left = Expr {
                node: ExprNode::Binary(op, Box::new(left), Box::new(right)),
                span,
                ty: None,
            };
        }
        Ok(left)
    }

    fn parse_unary(&mut self) -> Result<Expr, String> {
        let start_span = self.current_token().span;
        if self.match_token(&TokenKind::Minus) {
            let expr = self.parse_unary()?;
            let span = start_span.union(expr.span);
            Ok(Expr {
                node: ExprNode::Unary(UnaryOp::Neg, Box::new(expr)),
                span,
                ty: None,
            })
        } else if self.match_token(&TokenKind::Exclamation) {
            let expr = self.parse_unary()?;
            let span = start_span.union(expr.span);
            Ok(Expr {
                node: ExprNode::Unary(UnaryOp::Not, Box::new(expr)),
                span,
                ty: None,
            })
        } else if self.match_token(&TokenKind::Tilde) {
            let expr = self.parse_unary()?;
            let span = start_span.union(expr.span);
            Ok(Expr {
                node: ExprNode::Unary(UnaryOp::BitNot, Box::new(expr)),
                span,
                ty: None,
            })
        } else if self.match_token(&TokenKind::Star) {
            let expr = self.parse_unary()?;
            let span = start_span.union(expr.span);
            Ok(Expr {
                node: ExprNode::Unary(UnaryOp::Deref, Box::new(expr)),
                span,
                ty: None,
            })
        } else if self.match_token(&TokenKind::Ampersand) {
            let expr = self.parse_unary()?;
            let span = start_span.union(expr.span);
            Ok(Expr {
                node: ExprNode::Unary(UnaryOp::AddrOf, Box::new(expr)),
                span,
                ty: None,
            })
        } else if self.match_token(&TokenKind::PlusPlus) {
            let expr = self.parse_unary()?;
            let span = start_span.union(expr.span);
            Ok(Expr {
                node: ExprNode::Unary(UnaryOp::PreInc, Box::new(expr)),
                span,
                ty: None,
            })
        } else if self.match_token(&TokenKind::MinusMinus) {
            let expr = self.parse_unary()?;
            let span = start_span.union(expr.span);
            Ok(Expr {
                node: ExprNode::Unary(UnaryOp::PreDec, Box::new(expr)),
                span,
                ty: None,
            })
        } else if self.match_token(&TokenKind::Sizeof) {
            self.consume(&TokenKind::LParen, "Expected '(' after sizeof")?;
            // Determine if expression or type. Let's see: if first token is a type, parse type.
            let is_type = self.check(&TokenKind::Int)
                || self.check(&TokenKind::Char)
                || self.check(&TokenKind::Float)
                || self.check(&TokenKind::Double)
                || self.check(&TokenKind::Void)
                || self.check(&TokenKind::Struct)
                || self.check(&TokenKind::Union)
                || self.check(&TokenKind::Enum);
            let res = if is_type {
                let ty = self.parse_type()?;
                ExprNode::SizeofType(ty)
            } else {
                let expr = self.parse_expr()?;
                ExprNode::SizeofExpr(Box::new(expr))
            };
            self.consume(&TokenKind::RParen, "Expected ')'")?;
            let end_span = self.tokens[self.position - 1].span;
            Ok(Expr {
                node: res,
                span: start_span.union(end_span),
                ty: None,
            })
        } else if self.match_token(&TokenKind::Alignof) {
            self.consume(&TokenKind::LParen, "Expected '(' after alignof")?;
            let is_type = self.check(&TokenKind::Int)
                || self.check(&TokenKind::Char)
                || self.check(&TokenKind::Float)
                || self.check(&TokenKind::Double)
                || self.check(&TokenKind::Void)
                || self.check(&TokenKind::Struct)
                || self.check(&TokenKind::Union)
                || self.check(&TokenKind::Enum);
            let res = if is_type {
                let ty = self.parse_type()?;
                ExprNode::AlignofType(ty)
            } else {
                let expr = self.parse_expr()?;
                ExprNode::AlignofExpr(Box::new(expr))
            };
            self.consume(&TokenKind::RParen, "Expected ')'")?;
            let end_span = self.tokens[self.position - 1].span;
            Ok(Expr {
                node: res,
                span: start_span.union(end_span),
                ty: None,
            })
        } else if self.match_token(&TokenKind::LParen) {
            // Check if this is a Cast: `(type) expr`
            let is_type = self.check(&TokenKind::Int)
                || self.check(&TokenKind::Char)
                || self.check(&TokenKind::Float)
                || self.check(&TokenKind::Double)
                || self.check(&TokenKind::Void)
                || self.check(&TokenKind::Struct)
                || self.check(&TokenKind::Union)
                || self.check(&TokenKind::Enum)
                || self.check(&TokenKind::Const);

            if is_type {
                let cast_ty = self.parse_type()?;
                self.consume(&TokenKind::RParen, "Expected ')' after cast type")?;
                let expr = self.parse_unary()?;
                let span = start_span.union(expr.span);
                Ok(Expr {
                    node: ExprNode::Cast(cast_ty, Box::new(expr)),
                    span,
                    ty: None,
                })
            } else {
                // Just parenthesized expression
                let expr = self.parse_expr()?;
                self.consume(&TokenKind::RParen, "Expected ')' after expression")?;
                Ok(expr)
            }
        } else {
            self.parse_postfix()
        }
    }

    fn parse_postfix(&mut self) -> Result<Expr, String> {
        let mut expr = self.parse_primary()?;
        loop {
            let start_span = expr.span;
            if self.match_token(&TokenKind::LBracket) {
                // Array subscript: expr[idx] -> *(expr + idx)
                let idx = self.parse_expr()?;
                self.consume(&TokenKind::RBracket, "Expected ']'")?;
                let end_span = self.tokens[self.position - 1].span;
                
                // Represent array subscript as a dereference of addition
                let add_expr = Expr {
                    node: ExprNode::Binary(BinaryOp::Add, Box::new(expr), Box::new(idx)),
                    span: start_span.union(end_span),
                    ty: None,
                };
                expr = Expr {
                    node: ExprNode::Unary(UnaryOp::Deref, Box::new(add_expr)),
                    span: start_span.union(end_span),
                    ty: None,
                };
            } else if self.match_token(&TokenKind::LParen) {
                // Function call
                let mut args = Vec::new();
                if !self.check(&TokenKind::RParen) {
                    loop {
                        args.push(self.parse_expr()?);
                        if !self.match_token(&TokenKind::Comma) {
                            break;
                        }
                    }
                }
                self.consume(&TokenKind::RParen, "Expected ')' after arguments")?;
                let end_span = self.tokens[self.position - 1].span;
                expr = Expr {
                    node: ExprNode::Call(Box::new(expr), args),
                    span: start_span.union(end_span),
                    ty: None,
                };
            } else if self.match_token(&TokenKind::Dot) {
                // Member access
                let member_tok = self.consume(&TokenKind::Identifier(String::new()), "Expected member name")?;
                let member = match member_tok.kind {
                    TokenKind::Identifier(n) => n,
                    _ => unreachable!(),
                };
                let end_span = self.tokens[self.position - 1].span;
                expr = Expr {
                    node: ExprNode::Member(Box::new(expr), member, false),
                    span: start_span.union(end_span),
                    ty: None,
                };
            } else if self.match_token(&TokenKind::Arrow) {
                // Arrow access
                let member_tok = self.consume(&TokenKind::Identifier(String::new()), "Expected member name")?;
                let member = match member_tok.kind {
                    TokenKind::Identifier(n) => n,
                    _ => unreachable!(),
                };
                let end_span = self.tokens[self.position - 1].span;
                expr = Expr {
                    node: ExprNode::Member(Box::new(expr), member, true),
                    span: start_span.union(end_span),
                    ty: None,
                };
            } else if self.match_token(&TokenKind::PlusPlus) {
                let end_span = self.tokens[self.position - 1].span;
                expr = Expr {
                    node: ExprNode::Unary(UnaryOp::PostInc, Box::new(expr)),
                    span: start_span.union(end_span),
                    ty: None,
                };
            } else if self.match_token(&TokenKind::MinusMinus) {
                let end_span = self.tokens[self.position - 1].span;
                expr = Expr {
                    node: ExprNode::Unary(UnaryOp::PostDec, Box::new(expr)),
                    span: start_span.union(end_span),
                    ty: None,
                };
            } else {
                break;
            }
        }
        Ok(expr)
    }

    fn parse_primary(&mut self) -> Result<Expr, String> {
        let tok = self.advance().clone();
        match tok.kind {
            TokenKind::IntLiteral(val) => Ok(Expr {
                node: ExprNode::Literal(Literal::Int(val)),
                span: tok.span,
                ty: Some(Type::Int),
            }),
            TokenKind::FloatLiteral(val) => Ok(Expr {
                node: ExprNode::Literal(Literal::Float(val)),
                span: tok.span,
                ty: Some(Type::Double),
            }),
            TokenKind::CharLiteral(val) => Ok(Expr {
                node: ExprNode::Literal(Literal::Char(val)),
                span: tok.span,
                ty: Some(Type::Char),
            }),
            TokenKind::StringLiteral(val) => Ok(Expr {
                node: ExprNode::Literal(Literal::String(val)),
                span: tok.span,
                ty: Some(Type::Pointer(Box::new(Type::Char))),
            }),
            TokenKind::Nullptr => Ok(Expr {
                node: ExprNode::Literal(Literal::Nullptr),
                span: tok.span,
                ty: Some(Type::Nullptr),
            }),
            TokenKind::True => Ok(Expr {
                node: ExprNode::Literal(Literal::Bool(true)),
                span: tok.span,
                ty: Some(Type::Bool),
            }),
            TokenKind::False => Ok(Expr {
                node: ExprNode::Literal(Literal::Bool(false)),
                span: tok.span,
                ty: Some(Type::Bool),
            }),
            TokenKind::Identifier(val) => Ok(Expr {
                node: ExprNode::Identifier(val),
                span: tok.span,
                ty: None,
            }),
            _ => Err(format!("Expected primary expression, found {:?}", tok.kind)),
        }
    }
}
