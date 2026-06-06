use crate::ast::*;
use crate::error::{Span, Diagnostic};
use std::collections::{HashMap, HashSet};

pub struct SymbolTable<T> {
    scopes: Vec<HashMap<String, T>>,
}

impl<T: Clone> SymbolTable<T> {
    pub fn new() -> Self {
        Self {
            scopes: vec![HashMap::new()],
        }
    }

    pub fn enter_scope(&mut self) {
        self.scopes.push(HashMap::new());
    }

    pub fn exit_scope(&mut self) {
        self.scopes.pop();
    }

    pub fn insert(&mut self, name: String, val: T) {
        if let Some(scope) = self.scopes.last_mut() {
            scope.insert(name, val);
        }
    }

    pub fn lookup(&self, name: &str) -> Option<T> {
        for scope in self.scopes.iter().rev() {
            if let Some(val) = scope.get(name) {
                return Some(val.clone());
            }
        }
        None
    }

    pub fn lookup_current(&self, name: &str) -> Option<T> {
        self.scopes.last().and_then(|s| s.get(name).cloned())
    }
}

pub struct Typechecker {
    variables: SymbolTable<Type>,
    functions: HashMap<String, FunctionDecl>,
    structs: HashMap<String, StructDecl>,
    filename: String,
    pub errors: Vec<Diagnostic>,
    // Stack variable tracking for escape analysis
    // Maps a local variable to its scope depth
    local_vars: SymbolTable<usize>,
    scope_depth: usize,
}

impl Typechecker {
    pub fn new(filename: &str) -> Self {
        Self {
            variables: SymbolTable::new(),
            functions: HashMap::new(),
            structs: HashMap::new(),
            filename: filename.to_string(),
            errors: Vec::new(),
            local_vars: SymbolTable::new(),
            scope_depth: 0,
        }
    }

    pub fn check_program(&mut self, program: &mut Program) -> Result<(), Vec<Diagnostic>> {
        // First pass: Register structs and function signatures
        for decl in &program.decls {
            match decl {
                GlobalDecl::Struct(s) => {
                    self.structs.insert(s.name.clone(), s.clone());
                }
                GlobalDecl::Function(f) => {
                    self.functions.insert(f.name.clone(), f.clone());
                }
                _ => {}
            }
        }

        // Second pass: Full typechecking
        for decl in &mut program.decls {
            if let Err(err) = self.check_global_decl(decl) {
                self.errors.push(err);
            }
        }

        if self.errors.is_empty() {
            Ok(())
        } else {
            Err(self.errors.clone())
        }
    }

    fn check_global_decl(&mut self, decl: &mut GlobalDecl) -> Result<(), Diagnostic> {
        match decl {
            GlobalDecl::Struct(_) => Ok(()),
            GlobalDecl::GlobalVar(ty, name, init, span) => {
                if *ty == Type::Auto {
                    return Err(Diagnostic::error_with_span(
                        "Global variables cannot use 'auto' type inference",
                        *span,
                        "Global auto declaration",
                        &self.filename,
                    ));
                }
                self.variables.insert(name.clone(), ty.clone());
                if let Some(init_expr) = init {
                    let init_ty = self.check_expr(init_expr)?;
                    self.assert_assignable(ty, &init_ty, init_expr.span)?;
                }
                Ok(())
            }
            GlobalDecl::Function(f) => {
                self.variables.enter_scope();
                self.local_vars.enter_scope();
                self.scope_depth = 1;

                // Add parameters
                for param in &f.params {
                    self.variables.insert(param.name.clone(), param.ty.clone());
                    self.local_vars.insert(param.name.clone(), self.scope_depth);
                }

                if let Some(body) = &mut f.body {
                    self.check_stmt(body, &f.return_type)?;

                    // Definite Return Analysis
                    if f.return_type != Type::Void && !self.check_definite_return(body) {
                        return Err(Diagnostic::error_with_span(
                            format!("Function '{}' does not return a value on all control flow paths", f.name),
                            f.span,
                            "Missing return in non-void function",
                            &self.filename,
                        ));
                    }
                }

                self.scope_depth = 0;
                self.local_vars.exit_scope();
                self.variables.exit_scope();
                Ok(())
            }
        }
    }

    fn check_stmt(&mut self, stmt: &mut Stmt, return_ty: &Type) -> Result<(), Diagnostic> {
        match &mut stmt.node {
            StmtNode::Compound(stmts) => {
                self.variables.enter_scope();
                self.local_vars.enter_scope();
                self.scope_depth += 1;
                for s in stmts {
                    self.check_stmt(s, return_ty)?;
                }
                self.scope_depth -= 1;
                self.local_vars.exit_scope();
                self.variables.exit_scope();
                Ok(())
            }
            StmtNode::Expr(expr) => {
                self.check_expr(expr)?;
                Ok(())
            }
            StmtNode::Decl(ty, name, init) => {
                let mut var_ty = ty.clone();
                if let Some(init_expr) = init {
                    let init_ty = self.check_expr(init_expr)?;
                    if var_ty == Type::Auto {
                        var_ty = init_ty.clone();
                        *ty = var_ty.clone(); // Update AST type node
                    }
                    self.assert_assignable(&var_ty, &init_ty, init_expr.span)?;

                    // Escape analysis check: check if we are assigning a pointer to a shorter-lived local stack variable
                    if let ExprNode::Unary(UnaryOp::AddrOf, inner) = &init_expr.node {
                        if let ExprNode::Identifier(local_name) = &inner.node {
                            if let Some(depth) = self.local_vars.lookup(local_name) {
                                if depth > self.scope_depth {
                                    return Err(Diagnostic::error_with_span(
                                        "Address of local variable escapes its defining scope",
                                        init_expr.span,
                                        "Illegal address assignment",
                                        &self.filename,
                                    ));
                                }
                            }
                        }
                    }
                } else if var_ty == Type::Auto {
                    return Err(Diagnostic::error_with_span(
                        "auto variable must have an initializer",
                        stmt.span,
                        "auto without initializer",
                        &self.filename,
                    ));
                }

                self.variables.insert(name.clone(), var_ty);
                self.local_vars.insert(name.clone(), self.scope_depth);
                Ok(())
            }
            StmtNode::If(cond, then_branch, else_branch) => {
                let cond_ty = self.check_expr(cond)?;
                if !cond_ty.is_integer() && !cond_ty.is_pointer() {
                    return Err(Diagnostic::error_with_span(
                        "If condition must be of integer or pointer type",
                        cond.span,
                        "Invalid condition type",
                        &self.filename,
                    ));
                }
                self.check_stmt(then_branch, return_ty)?;
                if let Some(eb) = else_branch {
                    self.check_stmt(eb, return_ty)?;
                }
                Ok(())
            }
            StmtNode::While(cond, body) => {
                let cond_ty = self.check_expr(cond)?;
                if !cond_ty.is_integer() && !cond_ty.is_pointer() {
                    return Err(Diagnostic::error_with_span(
                        "While condition must be of integer or pointer type",
                        cond.span,
                        "Invalid condition type",
                        &self.filename,
                    ));
                }
                self.check_stmt(body, return_ty)?;
                Ok(())
            }
            StmtNode::For(init, cond, post, body) => {
                self.variables.enter_scope();
                self.local_vars.enter_scope();
                self.scope_depth += 1;

                if let Some(i) = init {
                    self.check_stmt(i, return_ty)?;
                }
                if let Some(c) = cond {
                    let cond_ty = self.check_expr(c)?;
                    if !cond_ty.is_integer() && !cond_ty.is_pointer() {
                        return Err(Diagnostic::error_with_span(
                            "For condition must be of integer or pointer type",
                            c.span,
                            "Invalid condition type",
                            &self.filename,
                        ));
                    }
                }
                if let Some(p) = post {
                    self.check_expr(p)?;
                }
                self.check_stmt(body, return_ty)?;

                self.scope_depth -= 1;
                self.local_vars.exit_scope();
                self.variables.exit_scope();
                Ok(())
            }
            StmtNode::Switch(cond, body) => {
                let cond_ty = self.check_expr(cond)?;
                if !cond_ty.is_integer() {
                    return Err(Diagnostic::error_with_span(
                        "Switch expression must be of integer type",
                        cond.span,
                        "Non-integer switch condition",
                        &self.filename,
                    ));
                }
                self.check_stmt(body, return_ty)?;
                Ok(())
            }
            StmtNode::Case(val, body) => {
                let val_ty = self.check_expr(val)?;
                if !val_ty.is_integer() {
                    return Err(Diagnostic::error_with_span(
                        "Case label must be an integer constant expression",
                        val.span,
                        "Non-integer case label",
                        &self.filename,
                    ));
                }
                self.check_stmt(body, return_ty)?;
                Ok(())
            }
            StmtNode::Default(body) => {
                self.check_stmt(body, return_ty)?;
                Ok(())
            }
            StmtNode::Break | StmtNode::Continue => Ok(()),
            StmtNode::Return(val) => {
                if let Some(expr) = val {
                    let expr_ty = self.check_expr(expr)?;
                    self.assert_assignable(return_ty, &expr_ty, expr.span)?;

                    // Escape analysis: returning address of a local variable
                    if let ExprNode::Unary(UnaryOp::AddrOf, inner) = &expr.node {
                        if let ExprNode::Identifier(name) = &inner.node {
                            if self.local_vars.lookup(name).is_some() {
                                return Err(Diagnostic::error_with_span(
                                    "Address of local variable escapes",
                                    expr.span,
                                    "Escaping stack address",
                                    &self.filename,
                                ));
                            }
                        }
                    }
                } else {
                    if *return_ty != Type::Void {
                        return Err(Diagnostic::error_with_span(
                            "Return in non-void function must return a value",
                            stmt.span,
                            "Missing return value",
                            &self.filename,
                        ));
                    }
                }
                Ok(())
            }
            StmtNode::Unsafe(body) => {
                self.check_stmt(body, return_ty)?;
                Ok(())
            }
        }
    }

    fn check_expr(&mut self, expr: &mut Expr) -> Result<Type, Diagnostic> {
        let ty = match &mut expr.node {
            ExprNode::Literal(lit) => match lit {
                Literal::Int(_) => Type::Int,
                Literal::Float(_) => Type::Double,
                Literal::Char(_) => Type::Char,
                Literal::String(_) => Type::Pointer(Box::new(Type::Char)),
                Literal::Nullptr => Type::Nullptr,
                Literal::Bool(_) => Type::Bool,
            },
            ExprNode::Identifier(name) => {
                if let Some(ty) = self.variables.lookup(name) {
                    ty
                } else if let Some(decl) = self.functions.get(name).cloned() {
                    Type::Pointer(Box::new(decl.return_type))
                } else {
                    return Err(Diagnostic::error_with_span(
                        format!("Use of undeclared identifier '{}'", name),
                        expr.span,
                        "Undeclared identifier",
                        &self.filename,
                    ));
                }
            }
            ExprNode::Binary(op, left, right) => {
                let left_ty = self.check_expr(left)?;
                let right_ty = self.check_expr(right)?;

                match op {
                    BinaryOp::Add => {
                        if left_ty.is_pointer() && right_ty.is_integer() {
                            left_ty
                        } else if left_ty.is_integer() && right_ty.is_pointer() {
                            right_ty
                        } else if left_ty.is_numeric() && right_ty.is_numeric() {
                            self.binary_promote(&left_ty, &right_ty)
                        } else {
                            return Err(Diagnostic::error_with_span(
                                "Invalid operands to addition",
                                expr.span,
                                "Operands must be numeric or pointer+integer",
                                &self.filename,
                            ));
                        }
                    }
                    BinaryOp::Sub => {
                        if left_ty.is_pointer() && right_ty.is_integer() {
                            left_ty
                        } else if left_ty.is_pointer() && right_ty.is_pointer() {
                            Type::Long // Pointer difference returns ptrdiff_t (Long)
                        } else if left_ty.is_numeric() && right_ty.is_numeric() {
                            self.binary_promote(&left_ty, &right_ty)
                        } else {
                            return Err(Diagnostic::error_with_span(
                                "Invalid operands to subtraction",
                                expr.span,
                                "Operands must be numeric or pointer-integer/pointer",
                                &self.filename,
                            ));
                        }
                    }
                    BinaryOp::Mul | BinaryOp::Div => {
                        if left_ty.is_numeric() && right_ty.is_numeric() {
                            self.binary_promote(&left_ty, &right_ty)
                        } else {
                            return Err(Diagnostic::error_with_span(
                                "Multiplicative operations require numeric types",
                                expr.span,
                                "Non-numeric operands",
                                &self.filename,
                            ));
                        }
                    }
                    BinaryOp::Mod => {
                        if left_ty.is_integer() && right_ty.is_integer() {
                            self.binary_promote(&left_ty, &right_ty)
                        } else {
                            return Err(Diagnostic::error_with_span(
                                "Modulo operation requires integer types",
                                expr.span,
                                "Non-integer modulo operands",
                                &self.filename,
                            ));
                        }
                    }
                    BinaryOp::Shl | BinaryOp::Shr => {
                        if left_ty.is_integer() && right_ty.is_integer() {
                            left_ty
                        } else {
                            return Err(Diagnostic::error_with_span(
                                "Shift operations require integer types",
                                expr.span,
                                "Non-integer shift operands",
                                &self.filename,
                            ));
                        }
                    }
                    BinaryOp::BitAnd | BinaryOp::BitOr | BinaryOp::BitXor => {
                        if left_ty.is_integer() && right_ty.is_integer() {
                            self.binary_promote(&left_ty, &right_ty)
                        } else {
                            return Err(Diagnostic::error_with_span(
                                "Bitwise operations require integer types",
                                expr.span,
                                "Non-integer bitwise operands",
                                &self.filename,
                            ));
                        }
                    }
                    BinaryOp::LogicalAnd | BinaryOp::LogicalOr => {
                        if (left_ty.is_integer() || left_ty.is_pointer()) && (right_ty.is_integer() || right_ty.is_pointer()) {
                            Type::Bool
                        } else {
                            return Err(Diagnostic::error_with_span(
                                "Logical operations require boolean/integer/pointer types",
                                expr.span,
                                "Invalid logical operands",
                                &self.filename,
                            ));
                        }
                    }
                    BinaryOp::Equal
                    | BinaryOp::NotEqual
                    | BinaryOp::Less
                    | BinaryOp::LessEqual
                    | BinaryOp::Greater
                    | BinaryOp::GreaterEqual => {
                        if left_ty.is_numeric() && right_ty.is_numeric() {
                            Type::Bool
                        } else if left_ty.is_pointer() && right_ty.is_pointer() {
                            Type::Bool
                        } else if left_ty.is_pointer() && right_ty == Type::Nullptr {
                            Type::Bool
                        } else if left_ty == Type::Nullptr && right_ty.is_pointer() {
                            Type::Bool
                        } else {
                            return Err(Diagnostic::error_with_span(
                                "Comparison operands must be of compatible numeric or pointer types",
                                expr.span,
                                "Incompatible comparison types",
                                &self.filename,
                            ));
                        }
                    }
                }
            }
            ExprNode::Unary(op, inner) => {
                let inner_ty = self.check_expr(inner)?;
                match op {
                    UnaryOp::Neg => {
                        if inner_ty.is_numeric() {
                            inner_ty
                        } else {
                            return Err(Diagnostic::error_with_span(
                                "Unary negation requires numeric operand",
                                expr.span,
                                "Non-numeric negation",
                                &self.filename,
                            ));
                        }
                    }
                    UnaryOp::Not => {
                        if inner_ty.is_integer() || inner_ty.is_pointer() {
                            Type::Bool
                        } else {
                            return Err(Diagnostic::error_with_span(
                                "Logical NOT requires integer or pointer operand",
                                expr.span,
                                "Invalid operand type",
                                &self.filename,
                            ));
                        }
                    }
                    UnaryOp::BitNot => {
                        if inner_ty.is_integer() {
                            inner_ty
                        } else {
                            return Err(Diagnostic::error_with_span(
                                "Bitwise NOT requires integer operand",
                                expr.span,
                                "Non-integer bitwise NOT",
                                &self.filename,
                            ));
                        }
                    }
                    UnaryOp::Deref => {
                        if let Type::Pointer(base) = inner_ty {
                            *base
                        } else {
                            return Err(Diagnostic::error_with_span(
                                "Cannot dereference non-pointer type",
                                expr.span,
                                "Non-pointer dereference",
                                &self.filename,
                            ));
                        }
                    }
                    UnaryOp::AddrOf => {
                        // Assert that the inner expr is an lvalue
                        if !self.is_lvalue(inner) {
                            return Err(Diagnostic::error_with_span(
                                "Cannot take the address of a non-lvalue expression",
                                expr.span,
                                "Address-of non-lvalue",
                                &self.filename,
                            ));
                        }
                        Type::Pointer(Box::new(inner_ty))
                    }
                    UnaryOp::PreInc | UnaryOp::PreDec | UnaryOp::PostInc | UnaryOp::PostDec => {
                        if !self.is_lvalue(inner) {
                            return Err(Diagnostic::error_with_span(
                                "Increment/decrement operand must be an lvalue",
                                expr.span,
                                "Non-lvalue operand",
                                &self.filename,
                            ));
                        }
                        if !inner_ty.is_integer() && !inner_ty.is_pointer() {
                            return Err(Diagnostic::error_with_span(
                                "Increment/decrement requires integer or pointer type",
                                expr.span,
                                "Invalid increment/decrement type",
                                &self.filename,
                            ));
                        }
                        inner_ty
                    }
                }
            }
            ExprNode::Assign(left, right) => {
                if !self.is_lvalue(left) {
                    return Err(Diagnostic::error_with_span(
                        "Left-hand side of assignment must be a modifiable lvalue",
                        left.span,
                        "Non-lvalue assignment target",
                        &self.filename,
                    ));
                }
                let left_ty = self.check_expr(left)?;
                let right_ty = self.check_expr(right)?;
                self.assert_assignable(&left_ty, &right_ty, right.span)?;
                left_ty
            }
            ExprNode::Call(callee, args) => {
                let callee_ty = self.check_expr(callee)?;
                if let ExprNode::Identifier(func_name) = &callee.node {
                    if let Some(decl) = self.functions.get(func_name).cloned() {
                        // Check arguments
                        if decl.is_variadic {
                            if args.len() < decl.params.len() {
                                return Err(Diagnostic::error_with_span(
                                    format!("Too few arguments to variadic function '{}'", func_name),
                                    expr.span,
                                    "Mismatched argument count",
                                    &self.filename,
                                ));
                            }
                        } else if args.len() != decl.params.len() {
                            return Err(Diagnostic::error_with_span(
                                format!("Function '{}' expects {} arguments, got {}", func_name, decl.params.len(), args.len()),
                                expr.span,
                                "Mismatched argument count",
                                &self.filename,
                            ));
                        }

                        // Verify formats for printf/scanf family
                        if func_name == "printf" || func_name == "sprintf" || func_name == "printf_s" {
                            if !args.is_empty() {
                                if let ExprNode::Literal(Literal::String(_)) = &args[0].node {
                                    // Valid constant format string
                                } else {
                                    return Err(Diagnostic::error_with_span(
                                        "Format string argument must be a compile-time string literal to prevent exploits",
                                        args[0].span,
                                        "Format string validation",
                                        &self.filename,
                                    ));
                                }
                            }
                        }

                        for (i, arg) in args.iter_mut().enumerate() {
                            let arg_ty = self.check_expr(arg)?;
                            if i < decl.params.len() {
                                self.assert_assignable(&decl.params[i].ty, &arg_ty, arg.span)?;
                            }
                        }
                        decl.return_type.clone()
                    } else {
                        return Err(Diagnostic::error_with_span(
                            format!("Calling undefined function '{}'", func_name),
                            expr.span,
                            "Undefined function call",
                            &self.filename,
                        ));
                    }
                } else if let Type::Pointer(inner) = callee_ty {
                    // Call via function pointer
                    // CFI check is generated in codegen, here we just return the return type
                    if let Type::Void = *inner {
                        Type::Void
                    } else {
                        // Fallback function signature
                        Type::Int
                    }
                } else {
                    return Err(Diagnostic::error_with_span(
                        "Callee is not a function or function pointer",
                        callee.span,
                        "Invalid callee type",
                        &self.filename,
                    ));
                }
            }
            ExprNode::Cast(cast_ty, inner) => {
                let inner_ty = self.check_expr(inner)?;
                // Casting safety checks
                if inner_ty.is_integer() && cast_ty.is_pointer() {
                    return Err(Diagnostic::error_with_span(
                        "Casting integer to pointer is disallowed in Safe C mode by default",
                        expr.span,
                        "Unsafe integer-to-pointer cast",
                        &self.filename,
                    ));
                }
                cast_ty.clone()
            }
            ExprNode::Member(inner, member_name, is_arrow) => {
                let mut inner_ty = self.check_expr(inner)?;
                if *is_arrow {
                    if let Type::Pointer(base) = inner_ty {
                        inner_ty = *base;
                    } else {
                        return Err(Diagnostic::error_with_span(
                            "LHS of '->' must be a pointer",
                            inner.span,
                            "Non-pointer arrow access",
                            &self.filename,
                        ));
                    }
                }

                if let Type::Struct(struct_name) = inner_ty {
                    if let Some(decl) = self.structs.get(&struct_name) {
                        if let Some(field) = decl.fields.iter().find(|f| f.name == *member_name) {
                            field.ty.clone()
                        } else {
                            return Err(Diagnostic::error_with_span(
                                format!("Struct '{}' has no member named '{}'", struct_name, member_name),
                                expr.span,
                                "Unknown struct field",
                                &self.filename,
                            ));
                        }
                    } else {
                        return Err(Diagnostic::error_with_span(
                            format!("Struct '{}' is not defined", struct_name),
                            expr.span,
                            "Undefined struct",
                            &self.filename,
                        ));
                    }
                } else {
                    return Err(Diagnostic::error_with_span(
                        "LHS of field access must be a struct type",
                        inner.span,
                        "Invalid field access target",
                        &self.filename,
                    ));
                }
            }
            ExprNode::SizeofExpr(_) | ExprNode::SizeofType(_) => Type::Long,
            ExprNode::AlignofExpr(_) | ExprNode::AlignofType(_) => Type::Long,
        };

        expr.ty = Some(ty.clone());
        Ok(ty)
    }

    fn is_lvalue(&self, expr: &Expr) -> bool {
        match &expr.node {
            ExprNode::Identifier(_) => true,
            ExprNode::Unary(UnaryOp::Deref, _) => true,
            ExprNode::Member(_, _, _) => true,
            _ => false,
        }
    }

    fn binary_promote(&self, left: &Type, right: &Type) -> Type {
        if *left == Type::Double || *right == Type::Double {
            Type::Double
        } else if *left == Type::Float || *right == Type::Float {
            Type::Float
        } else if *left == Type::UnsignedLong || *right == Type::UnsignedLong {
            Type::UnsignedLong
        } else if *left == Type::Long || *right == Type::Long {
            Type::Long
        } else if *left == Type::UnsignedInt || *right == Type::UnsignedInt {
            Type::UnsignedInt
        } else {
            Type::Int
        }
    }

    fn assert_assignable(&self, dest: &Type, src: &Type, span: Span) -> Result<(), Diagnostic> {
        if dest == src {
            return Ok(());
        }

        // Implicit pointer conversions
        if dest.is_pointer() && *src == Type::Nullptr {
            return Ok(());
        }

        // Implicit void* conversions
        if let Type::Pointer(inner_dest) = dest {
            if **inner_dest == Type::Void && src.is_pointer() {
                return Ok(());
            }
        }
        if let Type::Pointer(inner_src) = src {
            if **inner_src == Type::Void && dest.is_pointer() {
                return Ok(());
            }
        }

        // Implicit numeric conversions
        if dest.is_numeric() && src.is_numeric() {
            return Ok(());
        }

        Err(Diagnostic::error_with_span(
            format!("Incompatible types in assignment: cannot assign {:?} to {:?}", src, dest),
            span,
            "Type incompatibility",
            &self.filename,
        ))
    }

    // Definite Return Analysis
    fn check_definite_return(&self, stmt: &Stmt) -> bool {
        match &stmt.node {
            StmtNode::Return(_) => true,
            StmtNode::Compound(stmts) => {
                for s in stmts {
                    if self.check_definite_return(s) {
                        return true;
                    }
                }
                false
            }
            StmtNode::If(_, then_branch, else_branch) => {
                if let Some(eb) = else_branch {
                    self.check_definite_return(then_branch) && self.check_definite_return(eb)
                } else {
                    false
                }
            }
            StmtNode::While(_, body) => self.check_definite_return(body),
            StmtNode::For(_, _, _, body) => self.check_definite_return(body),
            StmtNode::Switch(_, body) => self.check_definite_return(body),
            StmtNode::Case(_, body) => self.check_definite_return(body),
            StmtNode::Default(body) => self.check_definite_return(body),
            StmtNode::Unsafe(body) => self.check_definite_return(body),
            _ => false,
        }
    }
}
