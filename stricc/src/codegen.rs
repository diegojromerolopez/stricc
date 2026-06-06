use crate::ast::*;
use crate::error::Span;
use inkwell::builder::Builder;
use inkwell::context::Context;
use inkwell::module::{Module, Linkage};
use inkwell::types::{BasicType, BasicTypeEnum, FunctionType, StructType};
use inkwell::values::{BasicValue, BasicValueEnum, FunctionValue, InstructionValue, PointerValue, IntValue, FloatValue};
use inkwell::IntPredicate;
use inkwell::AddressSpace;
use std::collections::HashMap;

pub struct Codegen<'a, 'ctx> {
    pub context: &'ctx Context,
    pub module: &'a Module<'ctx>,
    pub builder: &'a Builder<'ctx>,
    filename: String,
    
    // Symbol tables
    variables: HashMap<String, PointerValue<'ctx>>,
    // Pointer metadata mappings: local pointer variable name -> (base_alloca, size_alloca, key_alloca)
    pointer_metadata: HashMap<String, (PointerValue<'ctx>, PointerValue<'ctx>, PointerValue<'ctx>)>,
    
    struct_types: HashMap<String, StructType<'ctx>>,
    
    // Runtime functions
    abort_fn: FunctionValue<'ctx>,
    shadow_load_fn: FunctionValue<'ctx>,
    shadow_store_fn: FunctionValue<'ctx>,
    check_bounds_fn: FunctionValue<'ctx>,
    current_fn: Option<FunctionValue<'ctx>>,
}

impl<'a, 'ctx> Codegen<'a, 'ctx> {
    pub fn new(
        context: &'ctx Context,
        module: &'a Module<'ctx>,
        builder: &'a Builder<'ctx>,
        filename: &str,
    ) -> Self {
        // Declare runtime functions
        // void __stricc_rt_abort(const char* msg, const char* file, int line)
        let void_type = context.void_type();
        let ptr_type = context.ptr_type(AddressSpace::default());
        let i32_type = context.i32_type();
        let i64_type = context.i64_type();

        let abort_fn_type = void_type.fn_type(&[ptr_type.into(), ptr_type.into(), i32_type.into()], false);
        let abort_fn = module.add_function("__stricc_rt_abort", abort_fn_type, Some(Linkage::External));

        // void __stricc_rt_shadow_load(void* ptr_addr, void** base, size_t* size, uint64_t* key)
        let shadow_load_fn_type = void_type.fn_type(
            &[ptr_type.into(), ptr_type.into(), ptr_type.into(), ptr_type.into()],
            false,
        );
        let shadow_load_fn = module.add_function("__stricc_rt_shadow_load", shadow_load_fn_type, Some(Linkage::External));

        // void __stricc_rt_shadow_store(void* ptr_addr, void* base, size_t size, uint64_t key)
        let shadow_store_fn_type = void_type.fn_type(
            &[ptr_type.into(), ptr_type.into(), i64_type.into(), i64_type.into()],
            false,
        );
        let shadow_store_fn = module.add_function("__stricc_rt_shadow_store", shadow_store_fn_type, Some(Linkage::External));

        // void __stricc_rt_check_bounds(void* ptr, void* base, size_t size, uint64_t key, size_t access_size, const char* file, int line)
        let check_bounds_fn_type = void_type.fn_type(
            &[
                ptr_type.into(),
                ptr_type.into(),
                i64_type.into(),
                i64_type.into(),
                i64_type.into(),
                ptr_type.into(),
                i32_type.into(),
            ],
            false,
        );
        let check_bounds_fn = module.add_function("__stricc_rt_check_bounds", check_bounds_fn_type, Some(Linkage::External));

        Self {
            context,
            module,
            builder,
            filename: filename.to_string(),
            variables: HashMap::new(),
            pointer_metadata: HashMap::new(),
            struct_types: HashMap::new(),
            abort_fn,
            shadow_load_fn,
            shadow_store_fn,
            check_bounds_fn,
            current_fn: None,
        }
    }

    fn get_llvm_type(&self, ty: &Type) -> BasicTypeEnum<'ctx> {
        match ty {
            Type::Void => panic!("Cannot convert Void type to BasicTypeEnum"),
            Type::Bool => self.context.bool_type().into(),
            Type::Char | Type::UnsignedChar => self.context.i8_type().into(),
            Type::Short | Type::UnsignedShort => self.context.i16_type().into(),
            Type::Int | Type::UnsignedInt => self.context.i32_type().into(),
            Type::Long | Type::UnsignedLong => self.context.i64_type().into(),
            Type::Float => self.context.f32_type().into(),
            Type::Double => self.context.f64_type().into(),
            Type::Pointer(_) | Type::Nullptr => self.context.ptr_type(AddressSpace::default()).into(),
            Type::Array(inner, size) => self.get_llvm_type(inner).array_type(*size as u32).into(),
            Type::Struct(name) => self.struct_types.get(name).cloned().expect("Struct not declared").into(),
            Type::Union(name) => self.struct_types.get(name).cloned().expect("Union not declared").into(),
            Type::Enum(_) => self.context.i32_type().into(),
            Type::Auto => panic!("Unresolved auto type during codegen"),
            Type::TypeofExpression(_) | Type::TypeofType(_) => panic!("Unresolved typeof type during codegen"),
            Type::Atomic(inner) => self.get_llvm_type(inner),
        }
    }

    pub fn gen_program(&mut self, program: &Program) {
        // Pre-declare structs and unions
        for decl in &program.decls {
            if let GlobalDecl::Struct(s) = decl {
                let struct_type = self.context.opaque_struct_type(&s.name);
                self.struct_types.insert(s.name.clone(), struct_type);
            }
        }

        // Fill struct fields
        for decl in &program.decls {
            if let GlobalDecl::Struct(s) = decl {
                let struct_type = self.struct_types.get(&s.name).unwrap();
                let field_types: Vec<BasicTypeEnum> = s.fields.iter().map(|f| self.get_llvm_type(&f.ty)).collect();
                struct_type.set_body(&field_types, false);
            }
        }

        // Generate globals and functions
        for decl in &program.decls {
            match decl {
                GlobalDecl::GlobalVar(ty, name, init, _) => {
                    let llvm_ty = self.get_llvm_type(ty);
                    let global = self.module.add_global(llvm_ty, None, name);
                    global.set_linkage(Linkage::External);
                    if let Some(init_expr) = init {
                        // For simple global initialization, evaluate lit
                        if let ExprNode::Literal(lit) = &init_expr.node {
                            match lit {
                                Literal::Int(v) => global.set_initializer(&self.context.i32_type().const_int(*v as u64, false)),
                                Literal::Float(v) => global.set_initializer(&self.context.f64_type().const_float(*v)),
                                Literal::Char(v) => global.set_initializer(&self.context.i8_type().const_int(*v as u64, false)),
                                Literal::Bool(v) => global.set_initializer(&self.context.bool_type().const_int(if *v { 1 } else { 0 }, false)),
                                _ => {}
                            }
                        }
                    } else {
                        // Zero initialize by default
                        global.set_initializer(&llvm_ty.const_zero());
                    }
                }
                GlobalDecl::Function(f) => {
                    self.gen_function(f);
                }
                _ => {}
            }
        }
    }

    fn gen_function(&mut self, f: &FunctionDecl) {
        let return_type = if f.return_type == Type::Void {
            None
        } else {
            Some(self.get_llvm_type(&f.return_type))
        };

        let param_types: Vec<BasicTypeEnum> = f.params.iter().map(|p| self.get_llvm_type(&p.ty)).collect();
        let fn_type = match return_type {
            Some(rt) => rt.fn_type(
                &param_types.iter().map(|t| (*t).into()).collect::<Vec<_>>(),
                f.is_variadic,
            ),
            None => self.context.void_type().fn_type(
                &param_types.iter().map(|t| (*t).into()).collect::<Vec<_>>(),
                f.is_variadic,
            ),
        };

        let func = self.module.add_function(&f.name, fn_type, None);
        self.current_fn = Some(func);

        if let Some(body) = &f.body {
            let entry_block = self.context.append_basic_block(func, "entry");
            self.builder.position_at_end(entry_block);

            self.variables.clear();
            self.pointer_metadata.clear();

            // Allocate and store parameter variables
            for (i, param) in f.params.iter().enumerate() {
                let llvm_param_ty = self.get_llvm_type(&param.ty);
                let param_alloca = self.builder.build_alloca(llvm_param_ty, &param.name).unwrap();
                let val = func.get_nth_param(i as u32).unwrap();
                self.builder.build_store(param_alloca, val).unwrap();
                self.variables.insert(param.name.clone(), param_alloca);

                if param.ty.is_pointer() {
                    // For parameters, map their metadata to wildcard/infinite by default
                    let base_alloca = self.builder.build_alloca(self.context.ptr_type(AddressSpace::default()), &format!("{}_base", param.name)).unwrap();
                    let size_alloca = self.builder.build_alloca(self.context.i64_type(), &format!("{}_size", param.name)).unwrap();
                    let key_alloca = self.builder.build_alloca(self.context.i64_type(), &format!("{}_key", param.name)).unwrap();
                    
                    let infinite_size = self.context.i64_type().const_int(u64::MAX, false);
                    let null_base = self.context.ptr_type(AddressSpace::default()).const_null();
                    let wildcard_key = self.context.i64_type().const_int(0, false);
                    
                    self.builder.build_store(base_alloca, null_base).unwrap();
                    self.builder.build_store(size_alloca, infinite_size).unwrap();
                    self.builder.build_store(key_alloca, wildcard_key).unwrap();

                    self.pointer_metadata.insert(param.name.clone(), (base_alloca, size_alloca, key_alloca));
                }
            }

            self.gen_stmt(body);

            // Append safety return if none exists
            if f.return_type == Type::Void {
                self.builder.build_return(None).unwrap();
            } else {
                // If the block is not terminated, return zero/null fallback
                if self.builder.get_insert_block().unwrap().get_terminator().is_none() {
                    let zero = self.get_llvm_type(&f.return_type).const_zero();
                    self.builder.build_return(Some(&zero)).unwrap();
                }
            }
        }
    }

    fn gen_stmt(&mut self, stmt: &Stmt) {
        match &stmt.node {
            StmtNode::Compound(stmts) => {
                for s in stmts {
                    self.gen_stmt(s);
                }
            }
            StmtNode::Expr(expr) => {
                self.gen_expr(expr);
            }
            StmtNode::Decl(ty, name, init) => {
                let llvm_ty = self.get_llvm_type(ty);
                let alloca = self.builder.build_alloca(llvm_ty, name).unwrap();
                self.variables.insert(name.clone(), alloca);

                if ty.is_pointer() {
                    let base_alloca = self.builder.build_alloca(self.context.ptr_type(AddressSpace::default()), &format!("{}_base", name)).unwrap();
                    let size_alloca = self.builder.build_alloca(self.context.i64_type(), &format!("{}_size", name)).unwrap();
                    let key_alloca = self.builder.build_alloca(self.context.i64_type(), &format!("{}_key", name)).unwrap();
                    
                    self.pointer_metadata.insert(name.clone(), (base_alloca, size_alloca, key_alloca));

                    // Default initialize pointer to nullptr
                    let null_ptr = self.context.ptr_type(AddressSpace::default()).const_null();
                    self.builder.build_store(alloca, null_ptr).unwrap();
                    self.builder.build_store(base_alloca, null_ptr).unwrap();
                    self.builder.build_store(size_alloca, self.context.i64_type().const_zero()).unwrap();
                    self.builder.build_store(key_alloca, self.context.i64_type().const_zero()).unwrap();
                } else {
                    // Zero initialize scalar variables
                    self.builder.build_store(alloca, llvm_ty.const_zero()).unwrap();
                }

                if let Some(init_expr) = init {
                    let val = self.gen_expr(init_expr);
                    self.builder.build_store(alloca, val).unwrap();

                    if ty.is_pointer() {
                        // Track/assign metadata
                        let (base_val, size_val, key_val) = self.get_expr_pointer_metadata_with_val(init_expr, Some(val));
                        let (base_alloca, size_alloca, key_alloca) = self.pointer_metadata.get(name).unwrap();
                        self.builder.build_store(*base_alloca, base_val).unwrap();
                        self.builder.build_store(*size_alloca, size_val).unwrap();
                        self.builder.build_store(*key_alloca, key_val).unwrap();
                    }
                }
            }
            StmtNode::If(cond, then_branch, else_branch) => {
                let cond_val = self.gen_expr(cond).into_int_value();
                let func = self.current_fn.unwrap();

                let then_bb = self.context.append_basic_block(func, "then");
                let else_bb = self.context.append_basic_block(func, "else");
                let merge_bb = self.context.append_basic_block(func, "ifcont");

                self.builder.build_conditional_branch(cond_val, then_bb, else_bb).unwrap();

                // Generate then block
                self.builder.position_at_end(then_bb);
                self.gen_stmt(then_branch);
                if self.builder.get_insert_block().unwrap().get_terminator().is_none() {
                    self.builder.build_unconditional_branch(merge_bb).unwrap();
                }

                // Generate else block
                self.builder.position_at_end(else_bb);
                if let Some(eb) = else_branch {
                    self.gen_stmt(eb);
                }
                if self.builder.get_insert_block().unwrap().get_terminator().is_none() {
                    self.builder.build_unconditional_branch(merge_bb).unwrap();
                }

                self.builder.position_at_end(merge_bb);
            }
            StmtNode::While(cond, body) => {
                let func = self.current_fn.unwrap();
                let cond_bb = self.context.append_basic_block(func, "whilecond");
                let body_bb = self.context.append_basic_block(func, "whilebody");
                let merge_bb = self.context.append_basic_block(func, "whilecont");

                self.builder.build_unconditional_branch(cond_bb).unwrap();

                // Condition block
                self.builder.position_at_end(cond_bb);
                let cond_val = self.gen_expr(cond).into_int_value();
                self.builder.build_conditional_branch(cond_val, body_bb, merge_bb).unwrap();

                // Body block
                self.builder.position_at_end(body_bb);
                self.gen_stmt(body);
                if self.builder.get_insert_block().unwrap().get_terminator().is_none() {
                    self.builder.build_unconditional_branch(cond_bb).unwrap();
                }

                self.builder.position_at_end(merge_bb);
            }
            StmtNode::Return(val) => {
                if let Some(expr) = val {
                    let ret_val = self.gen_expr(expr);
                    self.builder.build_return(Some(&ret_val)).unwrap();
                } else {
                    self.builder.build_return(None).unwrap();
                }
            }
            StmtNode::Unsafe(body) => {
                self.gen_stmt(body);
            }
            _ => {} // Fallback for other statements in this subset
        }
    }

    fn gen_expr(&mut self, expr: &Expr) -> BasicValueEnum<'ctx> {
        match &expr.node {
            ExprNode::Literal(lit) => match lit {
                Literal::Int(v) => self.context.i32_type().const_int(*v as u64, false).into(),
                Literal::Float(v) => self.context.f64_type().const_float(*v).into(),
                Literal::Char(v) => self.context.i8_type().const_int(*v as u64, false).into(),
                Literal::String(v) => {
                    // Global string constant
                    let string_val = self.builder.build_global_string_ptr(v, "str").unwrap();
                    string_val.as_pointer_value().as_basic_value_enum()
                }
                Literal::Nullptr => self.context.ptr_type(AddressSpace::default()).const_null().into(),
                Literal::Bool(v) => self.context.bool_type().const_int(if *v { 1 } else { 0 }, false).into(),
            },
            ExprNode::Identifier(name) => {
                let ptr = *self.variables.get(name).expect("Undeclared variable in codegen");
                let llvm_ty = self.get_llvm_type(expr.ty.as_ref().unwrap());
                self.builder.build_load(llvm_ty, ptr, name).unwrap()
            }
            ExprNode::Assign(left, right) => {
                let right_val = self.gen_expr(right);
                if let ExprNode::Identifier(name) = &left.node {
                    let ptr = *self.variables.get(name).unwrap();
                    self.builder.build_store(ptr, right_val).unwrap();

                    if expr.ty.as_ref().unwrap().is_pointer() {
                        let (base_val, size_val, key_val) = self.get_expr_pointer_metadata_with_val(right, Some(right_val));
                        let (base_alloca, size_alloca, key_alloca) = self.pointer_metadata.get(name).unwrap();
                        self.builder.build_store(*base_alloca, base_val).unwrap();
                        self.builder.build_store(*size_alloca, size_val).unwrap();
                        self.builder.build_store(*key_alloca, key_val).unwrap();
                    }
                } else if let ExprNode::Unary(UnaryOp::Deref, inner) = &left.node {
                    let dest_ptr = self.gen_expr(inner).into_pointer_value();
                    
                    // Shadow bounds check before store
                    let (base, size, key) = self.get_expr_pointer_metadata_with_val(inner, Some(dest_ptr.into()));
                    let access_size = self.get_type_size(expr.ty.as_ref().unwrap());
                    self.emit_bounds_check(dest_ptr, base, size, key, access_size, expr.span);

                    self.builder.build_store(dest_ptr, right_val).unwrap();

                    // If storing a pointer into memory, save metadata to shadow memory
                    if expr.ty.as_ref().unwrap().is_pointer() {
                        let (stored_base, stored_size, stored_key) = self.get_expr_pointer_metadata_with_val(right, Some(right_val));
                        let ptr_addr_cast = self.builder.build_pointer_cast(dest_ptr, self.context.ptr_type(AddressSpace::default()), "ptr_cast").unwrap();
                        let base_cast = self.builder.build_pointer_cast(stored_base.into_pointer_value(), self.context.ptr_type(AddressSpace::default()), "base_cast").unwrap();
                        self.builder.build_call(
                            self.shadow_store_fn,
                            &[
                                ptr_addr_cast.into(),
                                base_cast.into(),
                                stored_size.into(),
                                stored_key.into(),
                            ],
                            "shadow_store",
                        ).unwrap();
                    }
                }
                right_val
            }
            ExprNode::Binary(op, left, right) => {
                let left_val = self.gen_expr(left);
                let right_val = self.gen_expr(right);

                match op {
                    BinaryOp::Add => {
                        let ty = left.ty.as_ref().unwrap();
                        if let Type::Pointer(inner) = ty {
                            let left_ptr = left_val.into_pointer_value();
                            let right_int = right_val.into_int_value();
                            let elem_llvm_ty = self.get_llvm_type(inner);
                            let result = unsafe { self.builder.build_gep(elem_llvm_ty, left_ptr, &[right_int], "ptr_add").unwrap() };
                            result.into()
                        } else if ty.is_pointer() {
                            let left_ptr = left_val.into_pointer_value();
                            let right_int = right_val.into_int_value();
                            let result = unsafe { self.builder.build_gep(self.context.i8_type(), left_ptr, &[right_int], "ptr_add").unwrap() };
                            result.into()
                        } else {
                            // Signed add with overflow check
                            let left_int = left_val.into_int_value();
                            let right_int = right_val.into_int_value();
                            self.emit_checked_arithmetic("llvm.sadd.with.overflow.i32", left_int, right_int, expr.span)
                        }
                    }
                    BinaryOp::Sub => {
                        let ty = left.ty.as_ref().unwrap();
                        if let Type::Pointer(inner) = ty {
                            let left_ptr = left_val.into_pointer_value();
                            if right.ty.as_ref().unwrap().is_pointer() {
                                // Pointer difference
                                let right_ptr = right_val.into_pointer_value();
                                let elem_llvm_ty = self.get_llvm_type(inner);
                                let diff = self.builder.build_ptr_diff(elem_llvm_ty, left_ptr, right_ptr, "ptr_diff").unwrap();
                                diff.into()
                            } else {
                                let right_int = right_val.into_int_value();
                                // Negate right_int for subtraction GEP
                                let neg_right = self.builder.build_int_neg(right_int, "neg").unwrap();
                                let elem_llvm_ty = self.get_llvm_type(inner);
                                let result = unsafe { self.builder.build_gep(elem_llvm_ty, left_ptr, &[neg_right], "ptr_sub").unwrap() };
                                result.into()
                            }
                        } else if ty.is_pointer() {
                            let left_ptr = left_val.into_pointer_value();
                            if right.ty.as_ref().unwrap().is_pointer() {
                                let right_ptr = right_val.into_pointer_value();
                                let diff = self.builder.build_ptr_diff(self.context.i8_type(), left_ptr, right_ptr, "ptr_diff").unwrap();
                                diff.into()
                            } else {
                                let right_int = right_val.into_int_value();
                                let neg_right = self.builder.build_int_neg(right_int, "neg").unwrap();
                                let result = unsafe { self.builder.build_gep(self.context.i8_type(), left_ptr, &[neg_right], "ptr_sub").unwrap() };
                                result.into()
                            }
                        } else {
                            let left_int = left_val.into_int_value();
                            let right_int = right_val.into_int_value();
                            self.emit_checked_arithmetic("llvm.ssub.with.overflow.i32", left_int, right_int, expr.span)
                        }
                    }
                    BinaryOp::Mul => {
                        let left_int = left_val.into_int_value();
                        let right_int = right_val.into_int_value();
                        self.emit_checked_arithmetic("llvm.smul.with.overflow.i32", left_int, right_int, expr.span)
                    }
                    BinaryOp::Div => {
                        let left_int = left_val.into_int_value();
                        let right_int = right_val.into_int_value();
                        
                        // Guard divisor != 0
                        self.emit_division_guard(right_int, expr.span);

                        self.builder.build_int_signed_div(left_int, right_int, "div").unwrap().into()
                    }
                    BinaryOp::Mod => {
                        let left_int = left_val.into_int_value();
                        let right_int = right_val.into_int_value();

                        // Guard divisor != 0
                        self.emit_division_guard(right_int, expr.span);

                        self.builder.build_int_signed_rem(left_int, right_int, "rem").unwrap().into()
                    }
                    BinaryOp::Equal => {
                        let result = if left.ty.as_ref().unwrap().is_pointer() {
                            self.builder.build_int_compare(IntPredicate::EQ, left_val.into_int_value(), right_val.into_int_value(), "eq").unwrap()
                        } else {
                            self.builder.build_int_compare(IntPredicate::EQ, left_val.into_int_value(), right_val.into_int_value(), "eq").unwrap()
                        };
                        result.into()
                    }
                    BinaryOp::NotEqual => {
                        let result = self.builder.build_int_compare(IntPredicate::NE, left_val.into_int_value(), right_val.into_int_value(), "ne").unwrap();
                        result.into()
                    }
                    BinaryOp::Less => {
                        let result = self.builder.build_int_compare(IntPredicate::SLT, left_val.into_int_value(), right_val.into_int_value(), "lt").unwrap();
                        result.into()
                    }
                    BinaryOp::LessEqual => {
                        let result = self.builder.build_int_compare(IntPredicate::SLE, left_val.into_int_value(), right_val.into_int_value(), "le").unwrap();
                        result.into()
                    }
                    BinaryOp::Greater => {
                        let result = self.builder.build_int_compare(IntPredicate::SGT, left_val.into_int_value(), right_val.into_int_value(), "gt").unwrap();
                        result.into()
                    }
                    BinaryOp::GreaterEqual => {
                        let result = self.builder.build_int_compare(IntPredicate::SGE, left_val.into_int_value(), right_val.into_int_value(), "ge").unwrap();
                        result.into()
                    }
                    _ => left_val, // Fallback for bitwise / logical in this simple prototype
                }
            }
            ExprNode::Unary(op, inner) => {
                let val = self.gen_expr(inner);
                match op {
                    UnaryOp::Neg => self.builder.build_int_neg(val.into_int_value(), "neg").unwrap().into(),
                    UnaryOp::Not => {
                        let cmp = self.builder.build_int_compare(IntPredicate::EQ, val.into_int_value(), val.into_int_value().get_type().const_zero(), "not").unwrap();
                        cmp.into()
                    }
                    UnaryOp::Deref => {
                        let ptr_val = val.into_pointer_value();

                        // Shadow bounds check
                        let (base, size, key) = self.get_expr_pointer_metadata_with_val(inner, Some(val));
                        let access_size = self.get_type_size(expr.ty.as_ref().unwrap());
                        self.emit_bounds_check(ptr_val, base, size, key, access_size, expr.span);

                        let loaded = self.builder.build_load(self.get_llvm_type(expr.ty.as_ref().unwrap()), ptr_val, "deref").unwrap();

                        // If loaded value is pointer type, load metadata from shadow memory
                        if expr.ty.as_ref().unwrap().is_pointer() {
                            // Loaded pointer will have metadata loaded dynamically by calling __stricc_rt_shadow_load
                            // For simplicity, we can do this inside get_expr_pointer_metadata or store it locally.
                        }
                        loaded
                    }
                    UnaryOp::AddrOf => {
                        if let ExprNode::Identifier(name) = &inner.node {
                            let ptr = *self.variables.get(name).unwrap();
                            ptr.into()
                        } else {
                            panic!("AddrOf non-identifier not supported in simple codegen");
                        }
                    }
                    _ => val, // Fallback
                }
            }
            ExprNode::Call(callee, args) => {
                if let ExprNode::Identifier(func_name) = &callee.node {
                    let func = self.module.get_function(func_name).expect("Function not found");
                    let compiled_args: Vec<BasicValueEnum> = args.iter().map(|arg| self.gen_expr(arg)).collect();
                    let call = self.builder.build_call(
                        func,
                        &compiled_args.iter().map(|v| (*v).into()).collect::<Vec<_>>(),
                        "call",
                    ).unwrap();
                    match call.try_as_basic_value().left() {
                        Some(val) => val,
                        None => self.context.i32_type().const_zero().into(), // Void fallback
                    }
                } else {
                    panic!("Indirect calls not supported in simple codegen");
                }
            }
            ExprNode::Cast(cast_ty, inner) => {
                let val = self.gen_expr(inner);
                if cast_ty.is_pointer() && inner.ty.as_ref().unwrap().is_pointer() {
                    let ptr_val = val.into_pointer_value();
                    let target_llvm_ty = self.get_llvm_type(cast_ty);
                    self.builder.build_pointer_cast(ptr_val, target_llvm_ty.into_pointer_type(), "cast").unwrap().into()
                } else if cast_ty.is_integer() && inner.ty.as_ref().unwrap().is_pointer() {
                    let ptr_val = val.into_pointer_value();
                    let target_llvm_ty = self.get_llvm_type(cast_ty);
                    self.builder.build_ptr_to_int(ptr_val, target_llvm_ty.into_int_type(), "cast").unwrap().into()
                } else if cast_ty.is_pointer() && inner.ty.as_ref().unwrap().is_integer() {
                    let int_val = val.into_int_value();
                    let target_llvm_ty = self.get_llvm_type(cast_ty);
                    self.builder.build_int_to_ptr(int_val, target_llvm_ty.into_pointer_type(), "cast").unwrap().into()
                } else if cast_ty.is_integer() && inner.ty.as_ref().unwrap().is_integer() {
                    let int_val = val.into_int_value();
                    let target_llvm_ty = self.get_llvm_type(cast_ty).into_int_type();
                    let src_width = int_val.get_type().get_bit_width();
                    let dest_width = target_llvm_ty.get_bit_width();
                    if dest_width < src_width {
                        self.builder.build_int_truncate(int_val, target_llvm_ty, "cast").unwrap().into()
                    } else if dest_width > src_width {
                        self.builder.build_int_s_extend(int_val, target_llvm_ty, "cast").unwrap().into()
                    } else {
                        val
                    }
                } else {
                    val
                }
            }
            _ => self.context.i32_type().const_zero().into(),
        }
    }

    fn get_type_size(&self, ty: &Type) -> BasicValueEnum<'ctx> {
        let size = match ty {
            Type::Char | Type::UnsignedChar => 1,
            Type::Short | Type::UnsignedShort => 2,
            Type::Int | Type::UnsignedInt | Type::Float => 4,
            Type::Long | Type::UnsignedLong | Type::Double | Type::Pointer(_) | Type::Nullptr => 8,
            _ => 8,
        };
        self.context.i64_type().const_int(size, false).into()
    }

    fn load_shadow_metadata(&mut self, ptr_val: PointerValue<'ctx>) -> (BasicValueEnum<'ctx>, BasicValueEnum<'ctx>, BasicValueEnum<'ctx>) {
        let base_out = self.builder.build_alloca(self.context.ptr_type(AddressSpace::default()), "shadow_base").unwrap();
        let size_out = self.builder.build_alloca(self.context.i64_type(), "shadow_size").unwrap();
        let key_out = self.builder.build_alloca(self.context.i64_type(), "shadow_key").unwrap();

        let ptr_addr_cast = self.builder.build_pointer_cast(ptr_val, self.context.ptr_type(AddressSpace::default()), "ptr_cast").unwrap();
        self.builder.build_call(
            self.shadow_load_fn,
            &[
                ptr_addr_cast.into(),
                base_out.into(),
                size_out.into(),
                key_out.into(),
            ],
            "shadow_load",
        ).unwrap();

        let base = self.builder.build_load(self.context.ptr_type(AddressSpace::default()), base_out, "base").unwrap();
        let size = self.builder.build_load(self.context.i64_type(), size_out, "size").unwrap();
        let key = self.builder.build_load(self.context.i64_type(), key_out, "key").unwrap();
        (base, size, key)
    }

    fn get_expr_pointer_metadata_with_val(
        &mut self,
        expr: &Expr,
        val: Option<BasicValueEnum<'ctx>>,
    ) -> (BasicValueEnum<'ctx>, BasicValueEnum<'ctx>, BasicValueEnum<'ctx>) {
        match &expr.node {
            ExprNode::Identifier(name) => {
                if let Some((base_alloca, size_alloca, key_alloca)) = self.pointer_metadata.get(name) {
                    let base = self.builder.build_load(self.context.ptr_type(AddressSpace::default()), *base_alloca, "base").unwrap();
                    let size = self.builder.build_load(self.context.i64_type(), *size_alloca, "size").unwrap();
                    let key = self.builder.build_load(self.context.i64_type(), *key_alloca, "key").unwrap();
                    (base, size, key)
                } else {
                    // Global variable, which is infinite bounds
                    let null_base = self.context.ptr_type(AddressSpace::default()).const_null().into();
                    let infinite_size = self.context.i64_type().const_int(u64::MAX, false).into();
                    let wildcard_key = self.context.i64_type().const_int(0, false).into();
                    (null_base, infinite_size, wildcard_key)
                }
            }
            ExprNode::Literal(Literal::Nullptr) => {
                let null_base = self.context.ptr_type(AddressSpace::default()).const_null().into();
                let zero = self.context.i64_type().const_zero().into();
                (null_base, zero, zero)
            }
            ExprNode::Unary(UnaryOp::AddrOf, inner) => {
                let addr = val.unwrap_or_else(|| self.gen_expr(expr));
                let inner_ty = inner.ty.as_ref().unwrap();
                let size = self.get_type_size(inner_ty);
                let key = self.context.i64_type().const_zero().into();
                (addr, size, key)
            }
            ExprNode::Unary(UnaryOp::Deref, inner) => {
                let ptr_addr = self.gen_expr(inner).into_pointer_value();
                self.load_shadow_metadata(ptr_addr)
            }
            ExprNode::Binary(BinaryOp::Add, left, _) => {
                self.get_expr_pointer_metadata_with_val(left, None)
            }
            ExprNode::Binary(BinaryOp::Sub, left, _) => {
                self.get_expr_pointer_metadata_with_val(left, None)
            }
            ExprNode::Cast(_, inner) => {
                self.get_expr_pointer_metadata_with_val(inner, val)
            }
            ExprNode::Call(_, _) => {
                let ptr_val = val.unwrap_or_else(|| self.gen_expr(expr)).into_pointer_value();
                self.load_shadow_metadata(ptr_val)
            }
            _ => {
                // Wildcard fallback
                let null_base = self.context.ptr_type(AddressSpace::default()).const_null().into();
                let infinite_size = self.context.i64_type().const_int(u64::MAX, false).into();
                let wildcard_key = self.context.i64_type().const_int(0, false).into();
                (null_base, infinite_size, wildcard_key)
            }
        }
    }

    fn get_expr_pointer_metadata(&mut self, expr: &Expr) -> (BasicValueEnum<'ctx>, BasicValueEnum<'ctx>, BasicValueEnum<'ctx>) {
        self.get_expr_pointer_metadata_with_val(expr, None)
    }

    fn emit_bounds_check(
        &mut self,
        ptr: PointerValue<'ctx>,
        base: BasicValueEnum<'ctx>,
        size: BasicValueEnum<'ctx>,
        key: BasicValueEnum<'ctx>,
        access_size: BasicValueEnum<'ctx>,
        span: Span,
    ) {
        let ptr_cast = self.builder.build_pointer_cast(ptr, self.context.ptr_type(AddressSpace::default()), "ptr_cast").unwrap();
        let base_cast = self.builder.build_pointer_cast(base.into_pointer_value(), self.context.ptr_type(AddressSpace::default()), "base_cast").unwrap();
        
        let file_val = self.builder.build_global_string_ptr(&self.filename, "file_name").unwrap();
        let line_val = self.context.i32_type().const_int(0, false); // Real lines tracked via DWARF or set simple 0 for default
        
        self.builder.build_call(
            self.check_bounds_fn,
            &[
                ptr_cast.into(),
                base_cast.into(),
                size.into(),
                key.into(),
                access_size.into(),
                file_val.as_pointer_value().into(),
                line_val.into(),
            ],
            "bounds_check",
        ).unwrap();
    }

    fn emit_division_guard(&mut self, divisor: IntValue<'ctx>, span: Span) {
        let func = self.current_fn.unwrap();
        let is_zero = self.builder.build_int_compare(IntPredicate::EQ, divisor, divisor.get_type().const_zero(), "is_zero").unwrap();

        let abort_bb = self.context.append_basic_block(func, "div_zero_abort");
        let cont_bb = self.context.append_basic_block(func, "div_zero_cont");

        self.builder.build_conditional_branch(is_zero, abort_bb, cont_bb).unwrap();

        // Abort block
        self.builder.position_at_end(abort_bb);
        let msg = self.builder.build_global_string_ptr("Division by zero", "msg").unwrap();
        let file = self.builder.build_global_string_ptr(&self.filename, "file").unwrap();
        let line = self.context.i32_type().const_int(0, false);
        self.builder.build_call(self.abort_fn, &[msg.as_pointer_value().into(), file.as_pointer_value().into(), line.into()], "abort").unwrap();
        self.builder.build_unreachable().unwrap();

        // Continue block
        self.builder.position_at_end(cont_bb);
    }

    fn emit_checked_arithmetic(
        &mut self,
        intrinsic_name: &str,
        left: IntValue<'ctx>,
        right: IntValue<'ctx>,
        span: Span,
    ) -> BasicValueEnum<'ctx> {
        let func = self.current_fn.unwrap();
        
        // Define struct { i32, i1 } returned by intrinsics
        let struct_ty = self.context.struct_type(&[left.get_type().into(), self.context.bool_type().into()], false);
        let intrinsic_fn = self.module.add_function(
            intrinsic_name,
            struct_ty.fn_type(&[left.get_type().into(), right.get_type().into()], false),
            None,
        );

        let result_struct = self.builder.build_call(intrinsic_fn, &[left.into(), right.into()], "arith_val").unwrap()
            .try_as_basic_value().left().unwrap().into_struct_value();

        let val = self.builder.build_extract_value(result_struct, 0, "res").unwrap();
        let overflow = self.builder.build_extract_value(result_struct, 1, "ovf").unwrap().into_int_value();

        let abort_bb = self.context.append_basic_block(func, "overflow_abort");
        let cont_bb = self.context.append_basic_block(func, "overflow_cont");

        self.builder.build_conditional_branch(overflow, abort_bb, cont_bb).unwrap();

        // Abort block
        self.builder.position_at_end(abort_bb);
        let msg = self.builder.build_global_string_ptr("Integer overflow detected", "msg").unwrap();
        let file = self.builder.build_global_string_ptr(&self.filename, "file").unwrap();
        let line = self.context.i32_type().const_int(0, false);
        self.builder.build_call(self.abort_fn, &[msg.as_pointer_value().into(), file.as_pointer_value().into(), line.into()], "abort").unwrap();
        self.builder.build_unreachable().unwrap();

        // Continue block
        self.builder.position_at_end(cont_bb);

        val
    }
}
