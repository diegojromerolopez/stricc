use crate::ast::*;
use crate::error::{Diagnostic, Span};
use std::collections::{HashMap, HashSet};

pub struct SymbolTable<T> {
    scopes: Vec<HashMap<String, T>>,
}

impl<T: Clone> Default for SymbolTable<T> {
    fn default() -> Self {
        Self::new()
    }
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
    unions: HashMap<String, StructDecl>,
    filename: String,
    pub errors: Vec<Diagnostic>,
    local_vars: SymbolTable<usize>,
    scope_depth: usize,
    in_unsafe: bool,
}

impl Typechecker {
    pub fn new(filename: &str) -> Self {
        Self {
            variables: SymbolTable::new(),
            functions: HashMap::new(),
            structs: HashMap::new(),
            unions: HashMap::new(),
            filename: filename.to_string(),
            errors: Vec::new(),
            local_vars: SymbolTable::new(),
            scope_depth: 0,
            in_unsafe: false,
        }
    }

    pub fn check_program(&mut self, program: &mut Program) -> Result<(), Vec<Diagnostic>> {
        // First pass: Register structs, unions and function signatures
        for decl in &program.decls {
            match decl {
                GlobalDecl::Struct(s) => {
                    self.structs.insert(s.name.clone(), s.clone());
                }
                GlobalDecl::Union(u) => {
                    self.unions.insert(u.name.clone(), u.clone());
                }
                GlobalDecl::Function(f) => {
                    if let Some(existing_decl) = self.functions.get(&f.name) {
                        if existing_decl.return_type != f.return_type
                            || existing_decl.params.len() != f.params.len()
                            || existing_decl.is_variadic != f.is_variadic
                            || existing_decl
                                .params
                                .iter()
                                .zip(f.params.iter())
                                .any(|(p1, p2)| p1.ty != p2.ty)
                        {
                            self.errors.push(Diagnostic::error_with_span(
                                format!("Conflicting redeclaration of function '{}'", f.name),
                                f.span,
                                "Conflicting redeclaration".to_string(),
                                &self.filename,
                            ));
                        }
                    } else {
                        self.functions.insert(f.name.clone(), f.clone());
                    }
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
            GlobalDecl::Union(_) => Ok(()),
            GlobalDecl::GlobalVar(ty, name, init, span) => {
                self.check_type(ty, *span)?;
                if *ty == Type::Auto {
                    return Err(Diagnostic::error_with_span(
                        "Global variables cannot use 'auto' type inference",
                        *span,
                        "Global auto declaration".to_string(),
                        &self.filename,
                    ));
                }
                if let Some(existing_ty) = self.variables.lookup(name) {
                    if existing_ty != *ty {
                        return Err(Diagnostic::error_with_span(
                            format!(
                                "Redeclaration of global variable '{name}' with conflicting type {ty:?}"
                            ),
                            *span,
                            "Conflicting redeclaration".to_string(),
                            &self.filename,
                        ));
                    }
                } else {
                    self.variables.insert(name.clone(), ty.clone());
                }
                if let Some(init_expr) = init {
                    let init_ty = self.check_expr(init_expr)?;
                    self.assert_assignable(ty, &init_ty, init_expr.span)?;
                }
                Ok(())
            }
            GlobalDecl::Function(f) => {
                self.check_type(&mut f.return_type, f.span)?;
                for param in &mut f.params {
                    self.check_type(&mut param.ty, f.span)?;
                }

                // Reject user-defined variadic function bodies (external libc declarations are fine)
                if f.is_variadic && f.body.is_some() {
                    return Err(Diagnostic::error_with_span(
                        format!("User-defined variadic function '{}' is forbidden in Safe C mode; only external variadic declarations (e.g. printf) are allowed", f.name),
                        f.span,
                        "Forbidden user-defined variadic".to_string(),
                        &self.filename,
                    ));
                }

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
                    if f.return_type != Type::Void && !Self::check_definite_return(body) {
                        return Err(Diagnostic::error_with_span(
                            format!(
                                "Function '{}' does not return a value on all control flow paths",
                                f.name
                            ),
                            f.span,
                            "Missing return in non-void function".to_string(),
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
                self.check_type(ty, stmt.span)?;
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
                } else if *return_ty != Type::Void {
                    return Err(Diagnostic::error_with_span(
                        "Return in non-void function must return a value",
                        stmt.span,
                        "Missing return value",
                        &self.filename,
                    ));
                }
                Ok(())
            }
            StmtNode::Unsafe(body) => {
                let prev_unsafe = self.in_unsafe;
                self.in_unsafe = true;
                self.check_stmt(body, return_ty)?;
                self.in_unsafe = prev_unsafe;
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
                    match ty {
                        Type::Array(inner, _) => Type::Pointer(inner),
                        _ => ty,
                    }
                } else if let Some(decl) = self.functions.get(name).cloned() {
                    Type::Pointer(Box::new(decl.return_type))
                } else {
                    return Err(Diagnostic::error_with_span(
                        format!("Use of undeclared identifier '{name}'"),
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
                            if let ExprNode::Literal(Literal::Int(val)) = &right.node {
                                let bit_width = match left_ty {
                                    Type::Bool => 1,
                                    Type::Char | Type::UnsignedChar => 8,
                                    Type::Short | Type::UnsignedShort => 16,
                                    Type::Int | Type::UnsignedInt => 32,
                                    Type::Long | Type::UnsignedLong => 64,
                                    _ => 32,
                                };
                                if *val < 0 || *val >= bit_width {
                                    return Err(Diagnostic::error_with_span(
                                        format!(
                                            "Shift count {val} is out of bounds for type {left_ty:?}"
                                        ),
                                        right.span,
                                        "Out-of-bounds constant shift",
                                        &self.filename,
                                    ));
                                }
                            }
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
                        if (left_ty.is_integer() || left_ty.is_pointer())
                            && (right_ty.is_integer() || right_ty.is_pointer())
                        {
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
                        if (left_ty.is_numeric() && right_ty.is_numeric())
                            || (left_ty.is_pointer() && right_ty.is_pointer())
                            || (left_ty.is_pointer() && right_ty == Type::Nullptr)
                            || (left_ty == Type::Nullptr && right_ty.is_pointer())
                        {
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
                let is_variable = if let ExprNode::Identifier(func_name) = &callee.node {
                    self.variables.lookup(func_name).is_some()
                } else {
                    false
                };

                if let ExprNode::Identifier(func_name) = &callee.node {
                    if !is_variable {
                        if let Some(decl) = self.functions.get(func_name).cloned() {
                            // Check arguments
                            if decl.is_variadic {
                                if args.len() < decl.params.len() {
                                    return Err(Diagnostic::error_with_span(
                                        format!(
                                            "Too few arguments to variadic function '{func_name}'"
                                        ),
                                        expr.span,
                                        "Mismatched argument count".to_string(),
                                        &self.filename,
                                    ));
                                }
                            } else if args.len() != decl.params.len() {
                                return Err(Diagnostic::error_with_span(
                                    format!(
                                        "Function '{}' expects {} arguments, got {}",
                                        func_name,
                                        decl.params.len(),
                                        args.len()
                                    ),
                                    expr.span,
                                    "Mismatched argument count".to_string(),
                                    &self.filename,
                                ));
                            }

                            // Verify formats for printf/scanf family
                            if func_name == "printf"
                                || func_name == "sprintf"
                                || func_name == "printf_s"
                            {
                                let fmt_arg_idx = if func_name == "sprintf" { 1 } else { 0 };
                                if args.len() > fmt_arg_idx {
                                    if let ExprNode::Literal(Literal::String(fmt)) =
                                        &args[fmt_arg_idx].node
                                    {
                                        let fmt_str = fmt.clone();
                                        self.validate_format_string(
                                            &fmt_str,
                                            args,
                                            fmt_arg_idx,
                                            expr.span,
                                        )?;
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
                                format!("Calling undefined function '{func_name}'"),
                                expr.span,
                                "Undefined function call".to_string(),
                                &self.filename,
                            ));
                        }
                    } else {
                        // It is a variable, treat it as a function pointer
                        if let Type::Pointer(inner) = callee_ty {
                            for arg in args.iter_mut() {
                                self.check_expr(arg)?;
                            }
                            if let Type::Void = *inner {
                                Type::Void
                            } else {
                                Type::Int
                            }
                        } else {
                            return Err(Diagnostic::error_with_span(
                                "Callee is not a function or function pointer",
                                callee.span,
                                "Invalid callee type".to_string(),
                                &self.filename,
                            ));
                        }
                    }
                } else if let Type::Pointer(inner) = callee_ty {
                    // Call via function pointer
                    for arg in args.iter_mut() {
                        self.check_expr(arg)?;
                    }
                    if let Type::Void = *inner {
                        Type::Void
                    } else {
                        Type::Int
                    }
                } else {
                    return Err(Diagnostic::error_with_span(
                        "Callee is not a function or function pointer",
                        callee.span,
                        "Invalid callee type".to_string(),
                        &self.filename,
                    ));
                }
            }
            ExprNode::Cast(cast_ty, inner) => {
                let inner_ty = self.check_expr(inner)?;
                // Casting safety checks
                if inner_ty.is_integer() && cast_ty.is_pointer() && !self.in_unsafe {
                    return Err(Diagnostic::error_with_span(
                        "Casting integer to pointer is disallowed in Safe C mode by default",
                        expr.span,
                        "Unsafe integer-to-pointer cast".to_string(),
                        &self.filename,
                    ));
                }
                // Casting pointer to integer loses bounds metadata and is disallowed
                if inner_ty.is_pointer() && cast_ty.is_integer() && !self.in_unsafe {
                    return Err(Diagnostic::error_with_span(
                        "Casting pointer to integer is disallowed in Safe C mode (use uintptr_t via unsafe block if strictly necessary)",
                        expr.span,
                        "Unsafe pointer-to-integer cast".to_string(),
                        &self.filename,
                    ));
                }
                if Self::has_const(&inner_ty) && !Self::has_const(cast_ty) {
                    return Err(Diagnostic::error_with_span(
                        "Casting away constness is disallowed in Safe C mode",
                        expr.span,
                        "Const qualifier discarded".to_string(),
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
                            "Non-pointer arrow access".to_string(),
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
                                format!(
                                    "Struct '{struct_name}' has no member named '{member_name}'"
                                ),
                                expr.span,
                                "Unknown struct field".to_string(),
                                &self.filename,
                            ));
                        }
                    } else {
                        return Err(Diagnostic::error_with_span(
                            format!("Struct '{struct_name}' is not defined"),
                            expr.span,
                            "Undefined struct".to_string(),
                            &self.filename,
                        ));
                    }
                } else if let Type::Union(union_name) = inner_ty {
                    if let Some(decl) = self.unions.get(&union_name) {
                        if let Some(field) = decl.fields.iter().find(|f| f.name == *member_name) {
                            field.ty.clone()
                        } else {
                            return Err(Diagnostic::error_with_span(
                                format!("Union '{union_name}' has no member named '{member_name}'"),
                                expr.span,
                                "Unknown union field".to_string(),
                                &self.filename,
                            ));
                        }
                    } else {
                        return Err(Diagnostic::error_with_span(
                            format!("Union '{union_name}' is not defined"),
                            expr.span,
                            "Undefined union".to_string(),
                            &self.filename,
                        ));
                    }
                } else {
                    return Err(Diagnostic::error_with_span(
                        "LHS of field access must be a struct or union type",
                        inner.span,
                        "Invalid field access target".to_string(),
                        &self.filename,
                    ));
                }
            }
            ExprNode::SizeofExpr(_) | ExprNode::SizeofType(_) => Type::Long,
            ExprNode::AlignofExpr(_) | ExprNode::AlignofType(_) => Type::Long,
        };

        expr.ty = Some(ty.clone());
        self.check_static_bounds(expr)?;
        self.check_sequence_points(expr)?;
        Ok(ty)
    }

    fn check_static_bounds(&self, expr: &Expr) -> Result<(), Diagnostic> {
        if let ExprNode::Unary(UnaryOp::Deref, inner) = &expr.node {
            if let ExprNode::Binary(BinaryOp::Add, left, right) = &inner.node {
                let check_array_index = |arr_expr: &Expr,
                                         idx_expr: &Expr|
                 -> Result<(), Diagnostic> {
                    if let ExprNode::Identifier(name) = &arr_expr.node {
                        if let Some(Type::Array(_, ArraySize::Const(len))) =
                            self.variables.lookup(name)
                        {
                            if let ExprNode::Literal(Literal::Int(idx_val)) = &idx_expr.node {
                                if *idx_val < 0 || *idx_val as usize >= len {
                                    return Err(Diagnostic::error_with_span(
                                        format!("Static out-of-bounds array access: index {idx_val} is out of bounds for array '{name}' of size {len}"),
                                        expr.span,
                                        "Array index out of bounds".to_string(),
                                        &self.filename,
                                    ));
                                }
                            }
                        }
                    }
                    Ok(())
                };
                check_array_index(left, right)?;
                check_array_index(right, left)?;
            }
        }
        Ok(())
    }

    fn is_lvalue(&self, expr: &Expr) -> bool {
        matches!(
            &expr.node,
            ExprNode::Identifier(_)
                | ExprNode::Unary(UnaryOp::Deref, _)
                | ExprNode::Member(_, _, _)
        )
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
        if let (Type::Pointer(inner_dest), Type::Pointer(inner_src)) = (dest, src) {
            if Self::unwrap_const(inner_dest) == Self::unwrap_const(inner_src) {
                if Self::has_const(inner_src) && !Self::has_const(inner_dest) {
                    return Err(Diagnostic::error_with_span(
                        format!("Incompatible pointer conversion: cannot implicitly discard const qualifier in assignment from {src:?} to {dest:?}"),
                        span,
                        "Type incompatibility",
                        &self.filename,
                    ));
                }
                return Ok(());
            }
        }

        // Implicit numeric conversions
        if dest.is_numeric() && src.is_numeric() {
            return Ok(());
        }

        Err(Diagnostic::error_with_span(
            format!("Incompatible types in assignment: cannot assign {src:?} to {dest:?}"),
            span,
            "Type incompatibility",
            &self.filename,
        ))
    }

    // Definite Return Analysis
    fn check_definite_return(stmt: &Stmt) -> bool {
        match &stmt.node {
            StmtNode::Return(_) => true,
            StmtNode::Compound(stmts) => {
                for s in stmts {
                    if Self::check_definite_return(s) {
                        return true;
                    }
                }
                false
            }
            StmtNode::If(_, then_branch, Some(else_branch)) => {
                Self::check_definite_return(then_branch) && Self::check_definite_return(else_branch)
            }
            StmtNode::While(_, body) => Self::check_definite_return(body),
            StmtNode::For(_, _, _, body) => Self::check_definite_return(body),
            StmtNode::Switch(_, body) => Self::check_definite_return(body),
            StmtNode::Case(_, body) => Self::check_definite_return(body),
            StmtNode::Default(body) => Self::check_definite_return(body),
            StmtNode::Unsafe(body) => Self::check_definite_return(body),
            _ => false,
        }
    }

    fn has_const(ty: &Type) -> bool {
        match ty {
            Type::Const(_) => true,
            Type::Pointer(inner) => Self::has_const(inner),
            Type::Array(inner, _) => Self::has_const(inner),
            _ => false,
        }
    }

    fn check_type(&mut self, ty: &mut Type, _span: Span) -> Result<(), Diagnostic> {
        match ty {
            Type::Pointer(inner) => self.check_type(inner, _span),
            Type::Array(inner, size) => {
                self.check_type(inner, _span)?;
                if let ArraySize::Variable(expr) = size {
                    let expr_ty = self.check_expr(expr)?;
                    if !expr_ty.is_integer() {
                        return Err(Diagnostic::error_with_span(
                            "Variable-length array size must be of integer type",
                            expr.span,
                            "Non-integer VLA size".to_string(),
                            &self.filename,
                        ));
                    }
                }
                Ok(())
            }
            Type::Const(inner) => self.check_type(inner, _span),
            Type::Atomic(inner) => self.check_type(inner, _span),
            _ => Ok(()),
        }
    }

    fn validate_format_string(
        &mut self,
        fmt: &str,
        args: &mut [Expr],
        fmt_arg_idx: usize,
        span: Span,
    ) -> Result<(), Diagnostic> {
        let mut specifiers = Vec::new();
        let mut chars = fmt.chars().peekable();
        while let Some(c) = chars.next() {
            if c == '%' {
                if let Some(&next_c) = chars.peek() {
                    if next_c == '%' {
                        chars.next();
                        continue;
                    }
                }
                let mut spec = String::new();
                while let Some(&next_c) = chars.peek() {
                    if next_c.is_alphabetic()
                        || next_c == '*'
                        || next_c == '.'
                        || next_c.is_ascii_digit()
                        || next_c == '-'
                        || next_c == '+'
                    {
                        spec.push(next_c);
                        chars.next();
                        if next_c.is_alphabetic() {
                            break;
                        }
                    } else {
                        break;
                    }
                }
                specifiers.push(spec);
            }
        }

        let mut arg_idx = fmt_arg_idx + 1;
        for spec in specifiers {
            if spec.contains('*') {
                if arg_idx >= args.len() {
                    return Err(Diagnostic::error_with_span(
                        "Mismatched printf arguments (missing argument for '*')".to_string(),
                        span,
                        "Format mismatch".to_string(),
                        &self.filename,
                    ));
                }
                let arg_ty = self.check_expr(&mut args[arg_idx])?;
                if !arg_ty.is_integer() {
                    return Err(Diagnostic::error_with_span(
                        format!("Expected integer for '*' specifier, found {arg_ty:?}"),
                        args[arg_idx].span,
                        "Format mismatch".to_string(),
                        &self.filename,
                    ));
                }
                arg_idx += 1;
            }

            if arg_idx >= args.len() {
                return Err(Diagnostic::error_with_span(
                    format!(
                        "Mismatched printf arguments: expected more arguments for specifier %{spec}"
                    ),
                    span,
                    "Format mismatch".to_string(),
                    &self.filename,
                ));
            }

            let arg_ty = self.check_expr(&mut args[arg_idx])?;
            let last_char = spec.chars().last().unwrap_or(' ');
            match last_char {
                'd' | 'i' | 'o' | 'u' | 'x' | 'X' | 'c' => {
                    if !arg_ty.is_integer() {
                        return Err(Diagnostic::error_with_span(
                            format!("Format specifier %{spec} expects integer, found {arg_ty:?}"),
                            args[arg_idx].span,
                            "Format mismatch".to_string(),
                            &self.filename,
                        ));
                    }
                }
                'f' | 'e' | 'E' | 'g' | 'G' => {
                    if !arg_ty.is_floating() {
                        return Err(Diagnostic::error_with_span(
                            format!(
                                "Format specifier %{spec} expects floating point, found {arg_ty:?}"
                            ),
                            args[arg_idx].span,
                            "Format mismatch".to_string(),
                            &self.filename,
                        ));
                    }
                }
                's' => {
                    if !arg_ty.is_pointer() {
                        return Err(Diagnostic::error_with_span(
                            format!(
                                "Format specifier %{spec} expects string pointer, found {arg_ty:?}"
                            ),
                            args[arg_idx].span,
                            "Format mismatch".to_string(),
                            &self.filename,
                        ));
                    }
                }
                'p' => {
                    if !arg_ty.is_pointer() {
                        return Err(Diagnostic::error_with_span(
                            format!("Format specifier %{spec} expects pointer, found {arg_ty:?}"),
                            args[arg_idx].span,
                            "Format mismatch".to_string(),
                            &self.filename,
                        ));
                    }
                }
                _ => {}
            }
            arg_idx += 1;
        }

        if arg_idx < args.len() {
            return Err(Diagnostic::error_with_span(
                "Mismatched printf arguments: too many arguments provided".to_string(),
                span,
                "Format mismatch".to_string(),
                &self.filename,
            ));
        }

        Ok(())
    }

    fn unwrap_const(ty: &Type) -> &Type {
        match ty {
            Type::Const(inner) => Self::unwrap_const(inner),
            _ => ty,
        }
    }

    fn check_sequence_points(&self, expr: &Expr) -> Result<(), Diagnostic> {
        self.get_effects(expr)?;
        Ok(())
    }

    fn check_conflicts(
        &self,
        span: Span,
        left_reads: &HashSet<String>,
        left_writes: &HashSet<String>,
        right_reads: &HashSet<String>,
        right_writes: &HashSet<String>,
    ) -> Result<(), Diagnostic> {
        for w in left_writes {
            if right_writes.contains(w) {
                return Err(Diagnostic::error_with_span(
                    format!("Sequence point violation: variable '{w}' is modified twice without a sequence point"),
                    span,
                    "Double write to variable".to_string(),
                    &self.filename,
                ));
            }
            if right_reads.contains(w) {
                return Err(Diagnostic::error_with_span(
                    format!("Sequence point violation: variable '{w}' is modified and read without a sequence point"),
                    span,
                    "Read-write conflict on variable".to_string(),
                    &self.filename,
                ));
            }
        }
        for w in right_writes {
            if left_reads.contains(w) {
                return Err(Diagnostic::error_with_span(
                    format!("Sequence point violation: variable '{w}' is modified and read without a sequence point"),
                    span,
                    "Read-write conflict on variable".to_string(),
                    &self.filename,
                ));
            }
        }
        Ok(())
    }

    fn get_effects(&self, expr: &Expr) -> Result<ExprEffects, Diagnostic> {
        match &expr.node {
            ExprNode::Literal(_) => Ok(ExprEffects {
                reads: HashSet::new(),
                writes: HashSet::new(),
            }),
            ExprNode::Identifier(name) => {
                let mut reads = HashSet::new();
                reads.insert(name.clone());
                Ok(ExprEffects {
                    reads,
                    writes: HashSet::new(),
                })
            }
            ExprNode::Unary(op, inner) => {
                let inner_eff = self.get_effects(inner)?;
                match op {
                    UnaryOp::PreInc | UnaryOp::PreDec | UnaryOp::PostInc | UnaryOp::PostDec => {
                        let mut writes = inner_eff.writes.clone();
                        let mut reads = inner_eff.reads.clone();
                        if let ExprNode::Identifier(name) = &inner.node {
                            writes.insert(name.clone());
                            reads.remove(name);
                        }
                        Ok(ExprEffects { reads, writes })
                    }
                    _ => Ok(inner_eff),
                }
            }
            ExprNode::Binary(op, left, right) => {
                let left_eff = self.get_effects(left)?;
                let right_eff = self.get_effects(right)?;
                if *op == BinaryOp::LogicalAnd || *op == BinaryOp::LogicalOr {
                    let mut reads = left_eff.reads.clone();
                    reads.extend(right_eff.reads.clone());
                    let mut writes = left_eff.writes.clone();
                    writes.extend(right_eff.writes.clone());
                    Ok(ExprEffects { reads, writes })
                } else {
                    self.check_conflicts(
                        expr.span,
                        &left_eff.reads,
                        &left_eff.writes,
                        &right_eff.reads,
                        &right_eff.writes,
                    )?;
                    let mut reads = left_eff.reads.clone();
                    reads.extend(right_eff.reads.clone());
                    let mut writes = left_eff.writes.clone();
                    writes.extend(right_eff.writes.clone());
                    Ok(ExprEffects { reads, writes })
                }
            }
            ExprNode::Assign(left, right) => {
                if let ExprNode::Identifier(name) = &left.node {
                    let right_eff = self.get_effects(right)?;
                    if right_eff.writes.contains(name) {
                        return Err(Diagnostic::error_with_span(
                            format!("Sequence point violation: variable '{name}' is modified twice without a sequence point"),
                            expr.span,
                            "Double write to variable".to_string(),
                            &self.filename,
                        ));
                    }
                    let mut writes = right_eff.writes.clone();
                    writes.insert(name.clone());
                    Ok(ExprEffects {
                        reads: right_eff.reads.clone(),
                        writes,
                    })
                } else {
                    let left_eff = self.get_effects(left)?;
                    let right_eff = self.get_effects(right)?;
                    self.check_conflicts(
                        expr.span,
                        &left_eff.reads,
                        &left_eff.writes,
                        &right_eff.reads,
                        &right_eff.writes,
                    )?;
                    let mut reads = left_eff.reads.clone();
                    reads.extend(right_eff.reads.clone());
                    let mut writes = left_eff.writes.clone();
                    writes.extend(right_eff.writes.clone());
                    Ok(ExprEffects { reads, writes })
                }
            }
            ExprNode::Call(callee, args) => {
                let callee_eff = self.get_effects(callee)?;
                let mut args_eff = Vec::new();
                for arg in args {
                    args_eff.push(self.get_effects(arg)?);
                }
                for arg_eff in &args_eff {
                    self.check_conflicts(
                        expr.span,
                        &callee_eff.reads,
                        &callee_eff.writes,
                        &arg_eff.reads,
                        &arg_eff.writes,
                    )?;
                }
                for i in 0..args_eff.len() {
                    for j in (i + 1)..args_eff.len() {
                        self.check_conflicts(
                            expr.span,
                            &args_eff[i].reads,
                            &args_eff[i].writes,
                            &args_eff[j].reads,
                            &args_eff[j].writes,
                        )?;
                    }
                }
                let mut reads = callee_eff.reads.clone();
                let mut writes = callee_eff.writes.clone();
                for arg_eff in args_eff {
                    reads.extend(arg_eff.reads);
                    writes.extend(arg_eff.writes);
                }
                Ok(ExprEffects { reads, writes })
            }
            ExprNode::Cast(_, inner) => self.get_effects(inner),
            ExprNode::Member(inner, _, _) => self.get_effects(inner),
            ExprNode::SizeofExpr(_)
            | ExprNode::SizeofType(_)
            | ExprNode::AlignofExpr(_)
            | ExprNode::AlignofType(_) => Ok(ExprEffects {
                reads: HashSet::new(),
                writes: HashSet::new(),
            }),
        }
    }
}

struct ExprEffects {
    reads: HashSet<String>,
    writes: HashSet<String>,
}
