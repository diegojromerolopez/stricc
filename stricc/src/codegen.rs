use crate::ast::*;
use crate::error::Span;
use inkwell::attributes::AttributeLoc;
use inkwell::builder::Builder;
use inkwell::context::Context;
use inkwell::module::{Linkage, Module};
use inkwell::types::{BasicType, BasicTypeEnum, IntType, StructType};
use inkwell::values::{
    BasicValue, BasicValueEnum, FloatValue, FunctionValue, IntValue, PointerValue,
};
use inkwell::AddressSpace;
use inkwell::IntPredicate;
use std::collections::HashMap;

pub struct Codegen<'a, 'ctx> {
    pub context: &'ctx Context,
    pub module: &'a Module<'ctx>,
    pub builder: &'a Builder<'ctx>,
    filename: String,

    // Symbol tables
    variables: HashMap<String, PointerValue<'ctx>>,
    variable_types: HashMap<String, Type>,
    // Pointer metadata mappings: local pointer variable name -> (base_alloca, size_alloca, key_alloca)
    pointer_metadata: HashMap<String, (PointerValue<'ctx>, PointerValue<'ctx>, PointerValue<'ctx>)>,
    stack_allocations: Vec<PointerValue<'ctx>>,

    struct_types: HashMap<String, StructType<'ctx>>,
    unions: HashMap<String, StructDecl>,
    structs: HashMap<String, StructDecl>,

    // Runtime functions
    abort_fn: FunctionValue<'ctx>,
    shadow_load_fn: FunctionValue<'ctx>,
    shadow_store_fn: FunctionValue<'ctx>,
    check_bounds_fn: FunctionValue<'ctx>,
    cfi_register_fn: FunctionValue<'ctx>,
    cfi_check_fn: FunctionValue<'ctx>,
    validate_printf_fn: FunctionValue<'ctx>,
    ffi_sandbox_in_fn: FunctionValue<'ctx>,
    ffi_sandbox_out_fn: FunctionValue<'ctx>,
    loop_barrier_fn: FunctionValue<'ctx>,
    ffi_enter_fn: FunctionValue<'ctx>,
    ffi_leave_fn: FunctionValue<'ctx>,
    register_stack_key_fn: FunctionValue<'ctx>,
    deregister_stack_key_fn: FunctionValue<'ctx>,
    get_next_key_fn: FunctionValue<'ctx>,
    #[allow(dead_code)]
    check_enum_range_fn: FunctionValue<'ctx>,
    variable_keys: HashMap<String, PointerValue<'ctx>>,
    function_locals: Vec<String>,
    block_locals: Vec<Vec<String>>,
    functions: HashMap<String, FunctionDecl>,
    current_fn: Option<FunctionValue<'ctx>>,
    #[allow(dead_code)]
    enums: HashMap<String, (i64, i64)>, // enum name -> (min_val, max_val)
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

        let abort_fn_type =
            void_type.fn_type(&[ptr_type.into(), ptr_type.into(), i32_type.into()], false);
        let abort_fn =
            module.add_function("__stricc_rt_abort", abort_fn_type, Some(Linkage::External));

        // void __stricc_rt_shadow_load(void* ptr_addr, void** base, size_t* size, uint64_t* key)
        let shadow_load_fn_type = void_type.fn_type(
            &[
                ptr_type.into(),
                ptr_type.into(),
                ptr_type.into(),
                ptr_type.into(),
            ],
            false,
        );
        let shadow_load_fn = module.add_function(
            "__stricc_rt_shadow_load",
            shadow_load_fn_type,
            Some(Linkage::External),
        );

        // void __stricc_rt_shadow_store(void* ptr_addr, void* base, size_t size, uint64_t key)
        let shadow_store_fn_type = void_type.fn_type(
            &[
                ptr_type.into(),
                ptr_type.into(),
                i64_type.into(),
                i64_type.into(),
            ],
            false,
        );
        let shadow_store_fn = module.add_function(
            "__stricc_rt_shadow_store",
            shadow_store_fn_type,
            Some(Linkage::External),
        );

        // void __stricc_rt_check_bounds(void* ptr, void* base, size_t size, uint64_t key, size_t access_size, bool is_write, const char* file, int line)
        let check_bounds_fn_type = void_type.fn_type(
            &[
                ptr_type.into(),
                ptr_type.into(),
                i64_type.into(),
                i64_type.into(),
                i64_type.into(),
                context.bool_type().into(), // is_write
                ptr_type.into(),
                i32_type.into(),
            ],
            false,
        );
        let check_bounds_fn = module.add_function(
            "__stricc_rt_check_bounds",
            check_bounds_fn_type,
            Some(Linkage::External),
        );
        let readonly_attr = context.create_string_attribute("readonly", "");
        let nounwind_attr = context.create_string_attribute("nounwind", "");
        check_bounds_fn.add_attribute(AttributeLoc::Function, readonly_attr);
        check_bounds_fn.add_attribute(AttributeLoc::Function, nounwind_attr);

        // void* __stricc_rt_ffi_sandbox_in(void* ptr, size_t size)
        let ffi_sandbox_in_fn_type = ptr_type.fn_type(&[ptr_type.into(), i64_type.into()], false);
        let ffi_sandbox_in_fn = module.add_function(
            "__stricc_rt_ffi_sandbox_in",
            ffi_sandbox_in_fn_type,
            Some(Linkage::External),
        );

        // void __stricc_rt_ffi_sandbox_out(void* dest_ptr, void* sandbox_ptr, size_t size)
        let ffi_sandbox_out_fn_type =
            void_type.fn_type(&[ptr_type.into(), ptr_type.into(), i64_type.into()], false);
        let ffi_sandbox_out_fn = module.add_function(
            "__stricc_rt_ffi_sandbox_out",
            ffi_sandbox_out_fn_type,
            Some(Linkage::External),
        );

        // void __stricc_rt_cfi_register(void* func_ptr, uint64_t signature_hash)
        let cfi_register_fn_type = void_type.fn_type(&[ptr_type.into(), i64_type.into()], false);
        let cfi_register_fn = module.add_function(
            "__stricc_rt_cfi_register",
            cfi_register_fn_type,
            Some(Linkage::External),
        );

        // void __stricc_rt_cfi_check(void* func_ptr, uint64_t expected_hash)
        let cfi_check_fn_type = void_type.fn_type(&[ptr_type.into(), i64_type.into()], false);
        let cfi_check_fn = module.add_function(
            "__stricc_rt_cfi_check",
            cfi_check_fn_type,
            Some(Linkage::External),
        );

        // void __stricc_rt_validate_printf(const char* fmt, int num_args, const int* type_ids)
        let validate_printf_fn_type =
            void_type.fn_type(&[ptr_type.into(), i32_type.into(), ptr_type.into()], false);
        let validate_printf_fn = module.add_function(
            "__stricc_rt_validate_printf",
            validate_printf_fn_type,
            Some(Linkage::External),
        );

        // void __stricc_rt_loop_barrier()
        let loop_barrier_fn_type = void_type.fn_type(&[], false);
        let loop_barrier_fn = module.add_function(
            "__stricc_rt_loop_barrier",
            loop_barrier_fn_type,
            Some(Linkage::External),
        );

        // void __stricc_rt_ffi_enter()
        let ffi_enter_fn_type = void_type.fn_type(&[], false);
        let ffi_enter_fn = module.add_function(
            "__stricc_rt_ffi_enter",
            ffi_enter_fn_type,
            Some(Linkage::External),
        );

        // void __stricc_rt_ffi_leave()
        let ffi_leave_fn_type = void_type.fn_type(&[], false);
        let ffi_leave_fn = module.add_function(
            "__stricc_rt_ffi_leave",
            ffi_leave_fn_type,
            Some(Linkage::External),
        );

        // void __stricc_rt_register_stack_key(void* addr, uint64_t key)
        let register_stack_key_fn_type =
            void_type.fn_type(&[ptr_type.into(), i64_type.into()], false);
        let register_stack_key_fn = module.add_function(
            "__stricc_rt_register_stack_key",
            register_stack_key_fn_type,
            Some(Linkage::External),
        );

        // void __stricc_rt_deregister_stack_key(void* addr)
        let deregister_stack_key_fn_type = void_type.fn_type(&[ptr_type.into()], false);
        let deregister_stack_key_fn = module.add_function(
            "__stricc_rt_deregister_stack_key",
            deregister_stack_key_fn_type,
            Some(Linkage::External),
        );

        // uint64_t __stricc_rt_get_next_key()
        let get_next_key_fn_type = i64_type.fn_type(&[], false);
        let get_next_key_fn = module.add_function(
            "__stricc_rt_get_next_key",
            get_next_key_fn_type,
            Some(Linkage::External),
        );

        // void __stricc_rt_check_enum_range(int32_t val, int32_t min_val, int32_t max_val, const char* file, int32_t line)
        let check_enum_range_fn_type = void_type.fn_type(
            &[
                i32_type.into(),
                i32_type.into(),
                i32_type.into(),
                ptr_type.into(),
                i32_type.into(),
            ],
            false,
        );
        let check_enum_range_fn = module.add_function(
            "__stricc_rt_check_enum_range",
            check_enum_range_fn_type,
            Some(Linkage::External),
        );

        Self {
            context,
            module,
            builder,
            filename: filename.to_string(),
            variables: HashMap::new(),
            variable_types: HashMap::new(),
            pointer_metadata: HashMap::new(),
            stack_allocations: Vec::new(),
            struct_types: HashMap::new(),
            unions: HashMap::new(),
            structs: HashMap::new(),
            functions: HashMap::new(),
            abort_fn,
            shadow_load_fn,
            shadow_store_fn,
            check_bounds_fn,
            cfi_register_fn,
            cfi_check_fn,
            validate_printf_fn,
            ffi_sandbox_in_fn,
            ffi_sandbox_out_fn,
            loop_barrier_fn,
            ffi_enter_fn,
            ffi_leave_fn,
            register_stack_key_fn,
            deregister_stack_key_fn,
            get_next_key_fn,
            check_enum_range_fn,
            variable_keys: HashMap::new(),
            function_locals: Vec::new(),
            block_locals: Vec::new(),
            current_fn: None,
            enums: HashMap::new(),
        }
    }

    fn create_entry_block_alloca<T: BasicType<'ctx>>(
        &self,
        ty: T,
        name: &str,
    ) -> PointerValue<'ctx> {
        let current_block = self.builder.get_insert_block().unwrap();
        let entry_block = self.current_fn.unwrap().get_first_basic_block().unwrap();
        match entry_block.get_first_instruction() {
            Some(first_instr) => self.builder.position_before(&first_instr),
            None => self.builder.position_at_end(entry_block),
        }
        let alloca = self.builder.build_alloca(ty, name).unwrap();
        self.builder.position_at_end(current_block);
        alloca
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
            Type::Pointer(_) | Type::Nullptr => {
                self.context.ptr_type(AddressSpace::default()).into()
            }
            Type::Array(inner, size) => match size {
                ArraySize::Const(len) => self.get_llvm_type(inner).array_type(*len as u32).into(),
                ArraySize::Variable(_) => self.context.ptr_type(AddressSpace::default()).into(),
            },
            Type::Struct(name) => self
                .struct_types
                .get(name)
                .cloned()
                .expect("Struct not declared")
                .into(),
            Type::Union(name) => self
                .struct_types
                .get(name)
                .cloned()
                .expect("Union not declared")
                .into(),
            Type::Enum(_) => self.context.i32_type().into(),
            Type::Auto => panic!("Unresolved auto type during codegen"),
            Type::TypeofExpression(_) | Type::TypeofType(_) => {
                panic!("Unresolved typeof type during codegen")
            }
            Type::Atomic(inner) => self.get_llvm_type(inner),
            Type::Const(inner) => self.get_llvm_type(inner),
        }
    }

    pub fn gen_program(&mut self, program: &Program) {
        // Populate structs, unions, and functions mapping
        for decl in &program.decls {
            match decl {
                GlobalDecl::Struct(s) => {
                    self.structs.insert(s.name.clone(), s.clone());
                }
                GlobalDecl::Union(u) => {
                    self.unions.insert(u.name.clone(), u.clone());
                }
                GlobalDecl::Function(f) => {
                    self.functions.insert(f.name.clone(), f.clone());
                }
                _ => {}
            }
        }

        // Pre-declare structs and unions
        for decl in &program.decls {
            match decl {
                GlobalDecl::Struct(s) => {
                    let struct_type = self.context.opaque_struct_type(&s.name);
                    self.struct_types.insert(s.name.clone(), struct_type);
                }
                GlobalDecl::Union(u) => {
                    let union_type = self.context.opaque_struct_type(&u.name);
                    self.struct_types.insert(u.name.clone(), union_type);
                }
                _ => {}
            }
        }

        // Fill struct and union bodies
        for decl in &program.decls {
            match decl {
                GlobalDecl::Struct(s) => {
                    let struct_type = self.struct_types.get(&s.name).unwrap();
                    let field_types: Vec<BasicTypeEnum> =
                        s.fields.iter().map(|f| self.get_llvm_type(&f.ty)).collect();
                    struct_type.set_body(&field_types, false);
                }
                GlobalDecl::Union(u) => {
                    let struct_type = self.struct_types.get(&u.name).unwrap();
                    let (total_size, max_align) =
                        self.get_type_size_and_align(&Type::Union(u.name.clone()));
                    let align_ty = if max_align >= 8 {
                        self.context.i64_type().into()
                    } else if max_align == 4 {
                        self.context.i32_type().into()
                    } else if max_align == 2 {
                        self.context.i16_type().into()
                    } else {
                        self.context.i8_type().into()
                    };
                    let align_size = max_align;
                    let body = if total_size <= align_size {
                        vec![align_ty]
                    } else {
                        let padding_size = total_size - align_size;
                        let padding_ty = self
                            .context
                            .i8_type()
                            .array_type(padding_size as u32)
                            .into();
                        vec![align_ty, padding_ty]
                    };
                    struct_type.set_body(&body, false);
                }
                _ => {}
            }
        }

        // Pre-declare all functions and collect those with a body
        let mut defined_functions = Vec::new();
        for decl in &program.decls {
            if let GlobalDecl::Function(f) = decl {
                let return_type = if f.return_type == Type::Void {
                    None
                } else {
                    Some(self.get_llvm_type(&f.return_type))
                };
                let param_types: Vec<BasicTypeEnum> =
                    f.params.iter().map(|p| self.get_llvm_type(&p.ty)).collect();
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

                if self.module.get_function(&f.name).is_none() {
                    self.module.add_function(&f.name, fn_type, None);
                }

                if f.body.is_some() {
                    let sig_hash = self.compute_signature_hash(
                        &f.return_type,
                        &f.params.iter().map(|p| p.ty.clone()).collect::<Vec<_>>(),
                    );
                    defined_functions.push((f.name.clone(), sig_hash));
                }
            }
        }

        // Generate globals and functions
        for decl in &program.decls {
            match decl {
                GlobalDecl::GlobalVar(ty, name, init, _) => {
                    let llvm_ty = self.get_llvm_type(ty);
                    let global = if let Some(g) = self.module.get_global(name) {
                        g
                    } else {
                        self.module.add_global(llvm_ty, None, name)
                    };
                    global.set_linkage(Linkage::External);
                    if let Some(init_expr) = init {
                        if let ExprNode::Literal(lit) = &init_expr.node {
                            match lit {
                                Literal::Int(v) => global.set_initializer(
                                    &self.context.i32_type().const_int(*v as u64, false),
                                ),
                                Literal::Float(v) => {
                                    global.set_initializer(&self.context.f64_type().const_float(*v))
                                }
                                Literal::Char(v) => global.set_initializer(
                                    &self.context.i8_type().const_int(*v as u64, false),
                                ),
                                Literal::Bool(v) => global.set_initializer(
                                    &self
                                        .context
                                        .bool_type()
                                        .const_int(if *v { 1 } else { 0 }, false),
                                ),
                                _ => {}
                            }
                        }
                    }
                }
                GlobalDecl::Function(f) => {
                    self.gen_function(f, &defined_functions);
                }
                _ => {}
            }
        }

        // Link-Time Type Validation: Generate defined/declared type signature symbols
        let mut sig_symbols: Vec<inkwell::values::PointerValue<'ctx>> = Vec::new();
        let i8_ty = self.context.i8_type();
        let ptr_ty = self.context.ptr_type(inkwell::AddressSpace::default());

        for decl in &program.decls {
            match decl {
                GlobalDecl::Function(f) => {
                    if self.is_std_lib_symbol(&f.name) {
                        continue;
                    }
                    let sig_hash = self.compute_signature_hash(
                        &f.return_type,
                        &f.params.iter().map(|p| p.ty.clone()).collect::<Vec<_>>(),
                    );
                    let sym_name = format!("__stricc_sig_fn_{}_{}", f.name, sig_hash);

                    if f.body.is_some() {
                        let global = self.module.add_global(i8_ty, None, &sym_name);
                        global.set_linkage(Linkage::External);
                        global.set_initializer(&i8_ty.const_int(0, false));
                        sig_symbols.push(global.as_pointer_value());
                    } else {
                        let global = self.module.add_global(i8_ty, None, &sym_name);
                        global.set_linkage(Linkage::External);
                        sig_symbols.push(global.as_pointer_value());
                    }
                }
                GlobalDecl::GlobalVar(ty, name, init, _) => {
                    if self.is_std_lib_symbol(name) {
                        continue;
                    }
                    let type_hash = self.compute_type_hash(ty);
                    let sym_name = format!("__stricc_sig_var_{name}_{type_hash}");

                    if init.is_some() {
                        let global = self.module.add_global(i8_ty, None, &sym_name);
                        global.set_linkage(Linkage::External);
                        global.set_initializer(&i8_ty.const_int(0, false));
                        sig_symbols.push(global.as_pointer_value());
                    } else {
                        let global = self.module.add_global(i8_ty, None, &sym_name);
                        global.set_linkage(Linkage::External);
                        sig_symbols.push(global.as_pointer_value());
                    }
                }
                _ => {}
            }
        }

        if !sig_symbols.is_empty() {
            let array_ty = ptr_ty.array_type(sig_symbols.len() as u32);
            let const_arr = ptr_ty.const_array(&sig_symbols);
            let refs_global = self
                .module
                .add_global(array_ty, None, "__stricc_references");
            refs_global.set_linkage(Linkage::Internal);
            refs_global.set_initializer(&const_arr);
        }
    }

    fn gen_function(&mut self, f: &FunctionDecl, defined_functions: &[(String, u64)]) {
        let func = self.module.get_function(&f.name).unwrap();
        self.current_fn = Some(func);

        let probe_stack_attr = self
            .context
            .create_string_attribute("probe-stack", "inline-asm");
        func.add_attribute(AttributeLoc::Function, probe_stack_attr);

        let probe_size_attr = self
            .context
            .create_string_attribute("stack-probe-size", "4096");
        func.add_attribute(AttributeLoc::Function, probe_size_attr);

        if let Some(body) = &f.body {
            let entry_block = self.context.append_basic_block(func, "entry");
            self.builder.position_at_end(entry_block);

            self.variables.clear();
            self.variable_types.clear();
            self.pointer_metadata.clear();
            self.stack_allocations.clear();
            self.variable_keys.clear();
            self.function_locals.clear();
            self.block_locals.clear();

            // Register all defined functions in main's entry block for CFI
            if f.name == "main" {
                let ensure_sig_fn = self
                    .module
                    .get_function("__stricc_rt_ensure_signal_handler")
                    .unwrap_or_else(|| {
                        let fn_type = self.context.void_type().fn_type(&[], false);
                        self.module.add_function(
                            "__stricc_rt_ensure_signal_handler",
                            fn_type,
                            Some(Linkage::External),
                        )
                    });
                self.builder
                    .build_call(ensure_sig_fn, &[], "sig_init")
                    .unwrap();

                for (fn_name, hash) in defined_functions {
                    if let Some(target_fn) = self.module.get_function(fn_name) {
                        let func_ptr = target_fn.as_global_value().as_pointer_value();
                        let func_ptr_cast = self
                            .builder
                            .build_pointer_cast(
                                func_ptr,
                                self.context.ptr_type(AddressSpace::default()),
                                "fn_ptr_cast",
                            )
                            .unwrap();
                        let hash_val = self.context.i64_type().const_int(*hash, false);
                        self.builder
                            .build_call(
                                self.cfi_register_fn,
                                &[func_ptr_cast.into(), hash_val.into()],
                                "cfi_reg",
                            )
                            .unwrap();
                    }
                }
            }

            // Allocate and store parameter variables
            for (i, param) in f.params.iter().enumerate() {
                let llvm_param_ty = self.get_llvm_type(&param.ty);
                let param_alloca = self.create_entry_block_alloca(llvm_param_ty, &param.name);
                let val = func.get_nth_param(i as u32).unwrap();
                self.builder.build_store(param_alloca, val).unwrap();
                self.variables.insert(param.name.clone(), param_alloca);
                self.variable_types
                    .insert(param.name.clone(), param.ty.clone());

                if param.ty.is_pointer() {
                    // For parameters, map their metadata to wildcard/infinite by default
                    let base_alloca = self.create_entry_block_alloca(
                        self.context.ptr_type(AddressSpace::default()),
                        &format!("{}_base", param.name),
                    );
                    let size_alloca = self.create_entry_block_alloca(
                        self.context.i64_type(),
                        &format!("{}_size", param.name),
                    );
                    let key_alloca = self.create_entry_block_alloca(
                        self.context.i64_type(),
                        &format!("{}_key", param.name),
                    );

                    let infinite_size = self.context.i64_type().const_int(u64::MAX, false);
                    let null_base = self.context.ptr_type(AddressSpace::default()).const_null();
                    let wildcard_key = self.context.i64_type().const_int(0, false);

                    self.builder.build_store(base_alloca, null_base).unwrap();
                    self.builder
                        .build_store(size_alloca, infinite_size)
                        .unwrap();
                    self.builder.build_store(key_alloca, wildcard_key).unwrap();

                    self.pointer_metadata
                        .insert(param.name.clone(), (base_alloca, size_alloca, key_alloca));
                }
            }

            self.gen_stmt(body);

            // Append safety return if none exists
            if f.return_type == Type::Void {
                if self
                    .builder
                    .get_insert_block()
                    .unwrap()
                    .get_terminator()
                    .is_none()
                {
                    let null_ptr = self.context.ptr_type(AddressSpace::default()).const_null();
                    let zero = self.context.i64_type().const_zero();
                    for &ptr in &self.stack_allocations {
                        let ptr_cast = self
                            .builder
                            .build_pointer_cast(
                                ptr,
                                self.context.ptr_type(AddressSpace::default()),
                                "ptr_cast",
                            )
                            .unwrap();
                        self.builder
                            .build_call(
                                self.shadow_store_fn,
                                &[ptr_cast.into(), null_ptr.into(), zero.into(), zero.into()],
                                "shadow_unreg",
                            )
                            .unwrap();
                    }
                    for name in &self.function_locals {
                        if let Some(alloca) = self.variables.get(name) {
                            let ptr_cast = self
                                .builder
                                .build_pointer_cast(
                                    *alloca,
                                    self.context.ptr_type(AddressSpace::default()),
                                    "ptr_cast",
                                )
                                .unwrap();
                            self.builder
                                .build_call(
                                    self.deregister_stack_key_fn,
                                    &[ptr_cast.into()],
                                    "stack_dereg",
                                )
                                .unwrap();
                        }
                    }
                    self.builder.build_return(None).unwrap();
                }
            } else {
                // If the block is not terminated, return zero/null fallback
                if self
                    .builder
                    .get_insert_block()
                    .unwrap()
                    .get_terminator()
                    .is_none()
                {
                    let null_ptr = self.context.ptr_type(AddressSpace::default()).const_null();
                    let zero = self.context.i64_type().const_zero();
                    for &ptr in &self.stack_allocations {
                        let ptr_cast = self
                            .builder
                            .build_pointer_cast(
                                ptr,
                                self.context.ptr_type(AddressSpace::default()),
                                "ptr_cast",
                            )
                            .unwrap();
                        self.builder
                            .build_call(
                                self.shadow_store_fn,
                                &[ptr_cast.into(), null_ptr.into(), zero.into(), zero.into()],
                                "shadow_unreg",
                            )
                            .unwrap();
                    }
                    for name in &self.function_locals {
                        if let Some(alloca) = self.variables.get(name) {
                            let ptr_cast = self
                                .builder
                                .build_pointer_cast(
                                    *alloca,
                                    self.context.ptr_type(AddressSpace::default()),
                                    "ptr_cast",
                                )
                                .unwrap();
                            self.builder
                                .build_call(
                                    self.deregister_stack_key_fn,
                                    &[ptr_cast.into()],
                                    "stack_dereg",
                                )
                                .unwrap();
                        }
                    }
                    let zero_val = self.get_llvm_type(&f.return_type).const_zero();
                    self.builder.build_return(Some(&zero_val)).unwrap();
                }
            }
        }
    }

    fn gen_stmt(&mut self, stmt: &Stmt) {
        match &stmt.node {
            StmtNode::Compound(stmts) => {
                self.block_locals.push(Vec::new());
                for s in stmts {
                    self.gen_stmt(s);
                }
                let locals = self.block_locals.pop().unwrap();
                for name in locals {
                    if let Some(alloca) = self.variables.get(&name) {
                        let ptr_cast = self
                            .builder
                            .build_pointer_cast(
                                *alloca,
                                self.context.ptr_type(AddressSpace::default()),
                                "ptr_cast",
                            )
                            .unwrap();
                        self.builder
                            .build_call(
                                self.deregister_stack_key_fn,
                                &[ptr_cast.into()],
                                "stack_dereg",
                            )
                            .unwrap();
                    }
                }
            }
            StmtNode::Expr(expr) => {
                self.gen_expr(expr);
            }
            StmtNode::Decl(ty, name, init) => {
                let alloca = match ty {
                    Type::Array(inner, ArraySize::Variable(size_expr)) => {
                        let func = self.current_fn.unwrap();
                        let size_val = self.gen_expr(size_expr).into_int_value();

                        // Check VLA size > 0
                        let is_le_zero = self
                            .builder
                            .build_int_compare(
                                IntPredicate::SLE,
                                size_val,
                                size_val.get_type().const_zero(),
                                "vla_size_le_zero",
                            )
                            .unwrap();

                        let abort_bb = self.context.append_basic_block(func, "vla_abort");
                        let cont_bb = self.context.append_basic_block(func, "vla_cont");
                        self.builder
                            .build_conditional_branch(is_le_zero, abort_bb, cont_bb)
                            .unwrap();

                        self.builder.position_at_end(abort_bb);
                        let msg = self
                            .builder
                            .build_global_string_ptr("Invalid VLA size", "msg")
                            .unwrap();
                        let file = self
                            .builder
                            .build_global_string_ptr(&self.filename, "file")
                            .unwrap();
                        let line = self.context.i32_type().const_int(0, false);
                        self.builder
                            .build_call(
                                self.abort_fn,
                                &[
                                    msg.as_pointer_value().into(),
                                    file.as_pointer_value().into(),
                                    line.into(),
                                ],
                                "abort",
                            )
                            .unwrap();
                        self.builder.build_unreachable().unwrap();

                        self.builder.position_at_end(cont_bb);

                        let inner_llvm_ty = self.get_llvm_type(inner);
                        let vla_alloca = self
                            .builder
                            .build_array_alloca(inner_llvm_ty, size_val, name)
                            .unwrap();

                        // Register VLA pointer in shadow table so bounds checks work on it
                        let (elem_sz, _) = self.get_type_size_and_align(inner);
                        let elem_size_val =
                            self.context.i64_type().const_int(elem_sz as u64, false);
                        let size_val_i64 = if size_val.get_type().get_bit_width() < 64 {
                            self.builder
                                .build_int_z_extend(
                                    size_val,
                                    self.context.i64_type(),
                                    "vla_size_ext",
                                )
                                .unwrap()
                        } else if size_val.get_type().get_bit_width() > 64 {
                            self.builder
                                .build_int_truncate(
                                    size_val,
                                    self.context.i64_type(),
                                    "vla_size_trunc",
                                )
                                .unwrap()
                        } else {
                            size_val
                        };
                        let vla_total_size = self
                            .builder
                            .build_int_mul(size_val_i64, elem_size_val, "vla_total_size")
                            .unwrap();
                        let vla_ptr_cast = self
                            .builder
                            .build_pointer_cast(
                                vla_alloca,
                                self.context.ptr_type(AddressSpace::default()),
                                "vla_ptr_cast",
                            )
                            .unwrap();
                        let vla_key = self
                            .builder
                            .build_call(self.get_next_key_fn, &[], "vla_key")
                            .unwrap()
                            .try_as_basic_value()
                            .left()
                            .unwrap();
                        self.builder
                            .build_call(
                                self.shadow_store_fn,
                                &[
                                    vla_ptr_cast.into(),
                                    vla_ptr_cast.into(),
                                    vla_total_size.into(),
                                    vla_key.into(),
                                ],
                                "vla_shadow_store",
                            )
                            .unwrap();

                        vla_alloca
                    }
                    _ => {
                        let llvm_ty = self.get_llvm_type(ty);
                        self.create_entry_block_alloca(llvm_ty, name)
                    }
                };

                self.variables.insert(name.clone(), alloca);
                self.variable_types.insert(name.clone(), ty.clone());

                // Allocate key slot for stack variable
                let key_alloca_var = self.create_entry_block_alloca(
                    self.context.i64_type(),
                    &format!("{name}_stack_key"),
                );
                let key_val = self
                    .builder
                    .build_call(self.get_next_key_fn, &[], "key_val")
                    .unwrap()
                    .try_as_basic_value()
                    .left()
                    .unwrap();
                self.builder.build_store(key_alloca_var, key_val).unwrap();
                self.variable_keys.insert(name.clone(), key_alloca_var);
                self.function_locals.push(name.clone());
                if !self.block_locals.is_empty() {
                    self.block_locals.last_mut().unwrap().push(name.clone());
                }

                // Register variable address in KEY_TABLE
                let var_ptr_cast = self
                    .builder
                    .build_pointer_cast(
                        alloca,
                        self.context.ptr_type(AddressSpace::default()),
                        "var_ptr_cast",
                    )
                    .unwrap();
                self.builder
                    .build_call(
                        self.register_stack_key_fn,
                        &[var_ptr_cast.into(), key_val.into()],
                        "stack_reg",
                    )
                    .unwrap();

                if ty.is_pointer() || matches!(ty, Type::Array(_, _)) {
                    let base_alloca = self.create_entry_block_alloca(
                        self.context.ptr_type(AddressSpace::default()),
                        &format!("{name}_base"),
                    );
                    let size_alloca = self.create_entry_block_alloca(
                        self.context.i64_type(),
                        &format!("{name}_size"),
                    );
                    let key_alloca = self
                        .create_entry_block_alloca(self.context.i64_type(), &format!("{name}_key"));

                    self.pointer_metadata
                        .insert(name.clone(), (base_alloca, size_alloca, key_alloca));

                    let array_ptr_cast = self
                        .builder
                        .build_pointer_cast(
                            alloca,
                            self.context.ptr_type(AddressSpace::default()),
                            "array_ptr_cast",
                        )
                        .unwrap();
                    self.builder
                        .build_store(base_alloca, array_ptr_cast)
                        .unwrap();
                    if matches!(ty, Type::Array(_, _)) {
                        self.builder.build_store(key_alloca, key_val).unwrap();
                    } else {
                        self.builder
                            .build_store(key_alloca, self.context.i64_type().const_zero())
                            .unwrap();
                    }

                    let total_size_val = match ty {
                        Type::Array(inner, ArraySize::Const(len)) => {
                            let (elem_sz, _) = self.get_type_size_and_align(inner);
                            let total_size = len * elem_sz;
                            let sz_val =
                                self.context.i64_type().const_int(total_size as u64, false);
                            self.builder.build_store(size_alloca, sz_val).unwrap();
                            sz_val
                        }
                        Type::Array(inner, ArraySize::Variable(size_expr)) => {
                            let size_val = self.gen_expr(size_expr).into_int_value();
                            let size_val_i64 = if size_val.get_type().get_bit_width() < 64 {
                                self.builder
                                    .build_int_z_extend(
                                        size_val,
                                        self.context.i64_type(),
                                        "size_extend",
                                    )
                                    .unwrap()
                            } else if size_val.get_type().get_bit_width() > 64 {
                                self.builder
                                    .build_int_truncate(
                                        size_val,
                                        self.context.i64_type(),
                                        "size_trunc",
                                    )
                                    .unwrap()
                            } else {
                                size_val
                            };
                            let (elem_sz, _) = self.get_type_size_and_align(inner);
                            let elem_size_val =
                                self.context.i64_type().const_int(elem_sz as u64, false);
                            let tot_sz = self
                                .builder
                                .build_int_mul(size_val_i64, elem_size_val, "vla_total_size")
                                .unwrap();
                            self.builder.build_store(size_alloca, tot_sz).unwrap();
                            tot_sz
                        }
                        _ => {
                            let null_ptr =
                                self.context.ptr_type(AddressSpace::default()).const_null();
                            self.builder.build_store(alloca, null_ptr).unwrap();
                            self.builder.build_store(base_alloca, null_ptr).unwrap();
                            self.builder
                                .build_store(size_alloca, self.context.i64_type().const_zero())
                                .unwrap();
                            self.context.i64_type().const_zero()
                        }
                    };

                    // Register in SHADOW_TABLE
                    let shadow_key_val = if matches!(ty, Type::Array(_, _)) {
                        key_val
                    } else {
                        self.context.i64_type().const_zero().into()
                    };
                    self.builder
                        .build_call(
                            self.shadow_store_fn,
                            &[
                                array_ptr_cast.into(),
                                array_ptr_cast.into(),
                                total_size_val.into(),
                                shadow_key_val.into(),
                            ],
                            "shadow_store",
                        )
                        .unwrap();

                    self.stack_allocations.push(alloca);
                } else {
                    // Zero initialize scalar variables
                    self.builder
                        .build_store(alloca, self.get_llvm_type(ty).const_zero())
                        .unwrap();
                }

                if let Some(init_expr) = init {
                    let val = self.gen_expr(init_expr);
                    self.builder.build_store(alloca, val).unwrap();

                    if ty.is_pointer() {
                        // Track/assign metadata
                        let (base_val, size_val, key_val) =
                            self.get_expr_pointer_metadata_with_val(init_expr, Some(val));
                        let (base_alloca, size_alloca, key_alloca) =
                            self.pointer_metadata.get(name).unwrap();
                        self.builder.build_store(*base_alloca, base_val).unwrap();
                        self.builder.build_store(*size_alloca, size_val).unwrap();
                        self.builder.build_store(*key_alloca, key_val).unwrap();
                    }
                }
            }
            StmtNode::If(cond, then_branch, else_branch) => {
                let cond_val = self.gen_expr(cond).into_int_value();
                let cond_val_i1 = if cond_val.get_type() == self.context.bool_type() {
                    cond_val
                } else {
                    self.builder
                        .build_int_compare(
                            IntPredicate::NE,
                            cond_val,
                            cond_val.get_type().const_zero(),
                            "cond_bool",
                        )
                        .unwrap()
                };
                let func = self.current_fn.unwrap();

                let then_bb = self.context.append_basic_block(func, "then");
                let else_bb = self.context.append_basic_block(func, "else");
                let merge_bb = self.context.append_basic_block(func, "ifcont");

                self.builder
                    .build_conditional_branch(cond_val_i1, then_bb, else_bb)
                    .unwrap();

                // Generate then block
                self.builder.position_at_end(then_bb);
                self.gen_stmt(then_branch);
                if self
                    .builder
                    .get_insert_block()
                    .unwrap()
                    .get_terminator()
                    .is_none()
                {
                    self.builder.build_unconditional_branch(merge_bb).unwrap();
                }

                // Generate else block
                self.builder.position_at_end(else_bb);
                if let Some(eb) = else_branch {
                    self.gen_stmt(eb);
                }
                if self
                    .builder
                    .get_insert_block()
                    .unwrap()
                    .get_terminator()
                    .is_none()
                {
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
                let cond_val_i1 = if cond_val.get_type() == self.context.bool_type() {
                    cond_val
                } else {
                    self.builder
                        .build_int_compare(
                            IntPredicate::NE,
                            cond_val,
                            cond_val.get_type().const_zero(),
                            "cond_bool",
                        )
                        .unwrap()
                };
                self.builder
                    .build_conditional_branch(cond_val_i1, body_bb, merge_bb)
                    .unwrap();

                // Body block
                self.builder.position_at_end(body_bb);
                self.builder
                    .build_call(self.loop_barrier_fn, &[], "loop_barrier")
                    .unwrap();
                self.gen_stmt(body);
                if self
                    .builder
                    .get_insert_block()
                    .unwrap()
                    .get_terminator()
                    .is_none()
                {
                    self.builder.build_unconditional_branch(cond_bb).unwrap();
                }

                self.builder.position_at_end(merge_bb);
            }
            StmtNode::For(init, cond, post, body) => {
                let func = self.current_fn.unwrap();
                let cond_bb = self.context.append_basic_block(func, "forcond");
                let body_bb = self.context.append_basic_block(func, "forbody");
                let post_bb = self.context.append_basic_block(func, "forpost");
                let merge_bb = self.context.append_basic_block(func, "forcont");

                // Init
                if let Some(i) = init {
                    self.gen_stmt(i);
                }
                self.builder.build_unconditional_branch(cond_bb).unwrap();

                // Condition
                self.builder.position_at_end(cond_bb);
                if let Some(c) = cond {
                    let cond_val = self.gen_expr(c).into_int_value();
                    let cond_val_i1 = if cond_val.get_type() == self.context.bool_type() {
                        cond_val
                    } else {
                        self.builder
                            .build_int_compare(
                                IntPredicate::NE,
                                cond_val,
                                cond_val.get_type().const_zero(),
                                "for_cond_bool",
                            )
                            .unwrap()
                    };
                    self.builder
                        .build_conditional_branch(cond_val_i1, body_bb, merge_bb)
                        .unwrap();
                } else {
                    // Unconditional loop (for(;;))
                    self.builder.build_unconditional_branch(body_bb).unwrap();
                }

                // Body — insert loop barrier to prevent LLVM from optimizing away side-effect-free loops
                self.builder.position_at_end(body_bb);
                self.builder
                    .build_call(self.loop_barrier_fn, &[], "loop_barrier")
                    .unwrap();
                self.gen_stmt(body);
                if self
                    .builder
                    .get_insert_block()
                    .unwrap()
                    .get_terminator()
                    .is_none()
                {
                    self.builder.build_unconditional_branch(post_bb).unwrap();
                }

                // Post (increment)
                self.builder.position_at_end(post_bb);
                if let Some(p) = post {
                    self.gen_expr(p);
                }
                self.builder.build_unconditional_branch(cond_bb).unwrap();

                self.builder.position_at_end(merge_bb);
            }
            StmtNode::Return(val) => {
                let null_ptr = self.context.ptr_type(AddressSpace::default()).const_null();
                let zero = self.context.i64_type().const_zero();
                for &ptr in &self.stack_allocations {
                    let ptr_cast = self
                        .builder
                        .build_pointer_cast(
                            ptr,
                            self.context.ptr_type(AddressSpace::default()),
                            "ptr_cast",
                        )
                        .unwrap();
                    self.builder
                        .build_call(
                            self.shadow_store_fn,
                            &[ptr_cast.into(), null_ptr.into(), zero.into(), zero.into()],
                            "shadow_unreg",
                        )
                        .unwrap();
                }
                for name in &self.function_locals {
                    if let Some(alloca) = self.variables.get(name) {
                        let ptr_cast = self
                            .builder
                            .build_pointer_cast(
                                *alloca,
                                self.context.ptr_type(AddressSpace::default()),
                                "ptr_cast",
                            )
                            .unwrap();
                        self.builder
                            .build_call(
                                self.deregister_stack_key_fn,
                                &[ptr_cast.into()],
                                "stack_dereg",
                            )
                            .unwrap();
                    }
                }

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
                Literal::Nullptr => self
                    .context
                    .ptr_type(AddressSpace::default())
                    .const_null()
                    .into(),
                Literal::Bool(v) => self
                    .context
                    .bool_type()
                    .const_int(if *v { 1 } else { 0 }, false)
                    .into(),
            },
            ExprNode::Identifier(name) => {
                if let Some(ptr) = self.variables.get(name) {
                    let ptr = *ptr;
                    let var_ty = self.variable_types.get(name).unwrap();
                    match var_ty {
                        Type::Array(_inner, size) => match size {
                            ArraySize::Const(_) => self
                                .builder
                                .build_pointer_cast(
                                    ptr,
                                    self.context.ptr_type(AddressSpace::default()),
                                    name,
                                )
                                .unwrap()
                                .into(),
                            ArraySize::Variable(_) => ptr.into(),
                        },
                        _ => {
                            self.build_load_and_sanitize_bool(expr.ty.as_ref().unwrap(), ptr, name)
                        }
                    }
                } else if let Some(target_fn) = self.module.get_function(name) {
                    let func_ptr = target_fn.as_global_value().as_pointer_value();
                    func_ptr.as_basic_value_enum()
                } else {
                    panic!("Undeclared variable or function '{name}' in codegen");
                }
            }
            ExprNode::Assign(left, right) => {
                let right_val = self.gen_expr(right);
                if let ExprNode::Identifier(name) = &left.node {
                    let ptr = *self.variables.get(name).unwrap();
                    self.builder.build_store(ptr, right_val).unwrap();

                    if expr.ty.as_ref().unwrap().is_pointer() {
                        let (base_val, size_val, key_val) =
                            self.get_expr_pointer_metadata_with_val(right, Some(right_val));
                        let (base_alloca, size_alloca, key_alloca) =
                            self.pointer_metadata.get(name).unwrap();
                        self.builder.build_store(*base_alloca, base_val).unwrap();
                        self.builder.build_store(*size_alloca, size_val).unwrap();
                        self.builder.build_store(*key_alloca, key_val).unwrap();
                    }
                } else if let ExprNode::Unary(UnaryOp::Deref, inner) = &left.node {
                    let dest_ptr = self.gen_expr(inner).into_pointer_value();

                    // Shadow bounds check before store
                    if !self.is_deref_statically_safe(inner) {
                        let (base, size, key) =
                            self.get_expr_pointer_metadata_with_val(inner, Some(dest_ptr.into()));
                        let access_size = self.get_type_size(expr.ty.as_ref().unwrap());
                        self.emit_bounds_check(
                            dest_ptr,
                            base,
                            size,
                            key,
                            access_size,
                            true,
                            expr.span,
                        );
                    }

                    // Alignment check before store
                    self.emit_alignment_check(dest_ptr, left.ty.as_ref().unwrap(), expr.span);

                    self.builder.build_store(dest_ptr, right_val).unwrap();

                    // If storing a pointer into memory, save metadata to shadow memory
                    if expr.ty.as_ref().unwrap().is_pointer() {
                        let (stored_base, stored_size, stored_key) =
                            self.get_expr_pointer_metadata_with_val(right, Some(right_val));
                        let ptr_addr_cast = self
                            .builder
                            .build_pointer_cast(
                                dest_ptr,
                                self.context.ptr_type(AddressSpace::default()),
                                "ptr_cast",
                            )
                            .unwrap();
                        let base_cast = self
                            .builder
                            .build_pointer_cast(
                                stored_base.into_pointer_value(),
                                self.context.ptr_type(AddressSpace::default()),
                                "base_cast",
                            )
                            .unwrap();
                        self.builder
                            .build_call(
                                self.shadow_store_fn,
                                &[
                                    ptr_addr_cast.into(),
                                    base_cast.into(),
                                    stored_size.into(),
                                    stored_key.into(),
                                ],
                                "shadow_store",
                            )
                            .unwrap();
                    }
                } else if let ExprNode::Member(inner, member_name, is_arrow) = &left.node {
                    let member_ptr = self.gen_member_pointer(inner, member_name, *is_arrow);
                    let member_ty = left.ty.as_ref().unwrap();

                    // Alignment check before store
                    self.emit_alignment_check(member_ptr, member_ty, expr.span);

                    self.builder.build_store(member_ptr, right_val).unwrap();

                    // If storing a pointer into memory, save metadata to shadow memory
                    if expr.ty.as_ref().unwrap().is_pointer() {
                        let (stored_base, stored_size, stored_key) =
                            self.get_expr_pointer_metadata_with_val(right, Some(right_val));
                        let ptr_addr_cast = self
                            .builder
                            .build_pointer_cast(
                                member_ptr,
                                self.context.ptr_type(AddressSpace::default()),
                                "ptr_cast",
                            )
                            .unwrap();
                        let base_cast = self
                            .builder
                            .build_pointer_cast(
                                stored_base.into_pointer_value(),
                                self.context.ptr_type(AddressSpace::default()),
                                "base_cast",
                            )
                            .unwrap();
                        self.builder
                            .build_call(
                                self.shadow_store_fn,
                                &[
                                    ptr_addr_cast.into(),
                                    base_cast.into(),
                                    stored_size.into(),
                                    stored_key.into(),
                                ],
                                "shadow_store",
                            )
                            .unwrap();
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
                            let result = unsafe {
                                self.builder
                                    .build_gep(elem_llvm_ty, left_ptr, &[right_int], "ptr_add")
                                    .unwrap()
                            };
                            result.into()
                        } else if ty.is_pointer() {
                            let left_ptr = left_val.into_pointer_value();
                            let right_int = right_val.into_int_value();
                            let result = unsafe {
                                self.builder
                                    .build_gep(
                                        self.context.i8_type(),
                                        left_ptr,
                                        &[right_int],
                                        "ptr_add",
                                    )
                                    .unwrap()
                            };
                            result.into()
                        } else if ty.is_floating() {
                            let res = self
                                .builder
                                .build_float_add(
                                    left_val.into_float_value(),
                                    right_val.into_float_value(),
                                    "fadd",
                                )
                                .unwrap();
                            self.emit_float_overflow_check(res, expr.span);
                            res.into()
                        } else {
                            let left_int = left_val.into_int_value();
                            let right_int = right_val.into_int_value();
                            if self.is_signed_type(ty) {
                                self.emit_checked_arithmetic(
                                    "llvm.sadd.with.overflow.i32",
                                    left_int,
                                    right_int,
                                    expr.span,
                                )
                            } else {
                                self.builder
                                    .build_int_add(left_int, right_int, "add")
                                    .unwrap()
                                    .into()
                            }
                        }
                    }
                    BinaryOp::Sub => {
                        let ty = left.ty.as_ref().unwrap();
                        if let Type::Pointer(inner) = ty {
                            let left_ptr = left_val.into_pointer_value();
                            if right.ty.as_ref().unwrap().is_pointer() {
                                let right_ptr = right_val.into_pointer_value();
                                let elem_llvm_ty = self.get_llvm_type(inner);
                                let diff = self
                                    .builder
                                    .build_ptr_diff(elem_llvm_ty, left_ptr, right_ptr, "ptr_diff")
                                    .unwrap();
                                diff.into()
                            } else {
                                let right_int = right_val.into_int_value();
                                let neg_right =
                                    self.builder.build_int_neg(right_int, "neg").unwrap();
                                let elem_llvm_ty = self.get_llvm_type(inner);
                                let result = unsafe {
                                    self.builder
                                        .build_gep(elem_llvm_ty, left_ptr, &[neg_right], "ptr_sub")
                                        .unwrap()
                                };
                                result.into()
                            }
                        } else if ty.is_pointer() {
                            let left_ptr = left_val.into_pointer_value();
                            if right.ty.as_ref().unwrap().is_pointer() {
                                let right_ptr = right_val.into_pointer_value();
                                let diff = self
                                    .builder
                                    .build_ptr_diff(
                                        self.context.i8_type(),
                                        left_ptr,
                                        right_ptr,
                                        "ptr_diff",
                                    )
                                    .unwrap();
                                diff.into()
                            } else {
                                let right_int = right_val.into_int_value();
                                let neg_right =
                                    self.builder.build_int_neg(right_int, "neg").unwrap();
                                let result = unsafe {
                                    self.builder
                                        .build_gep(
                                            self.context.i8_type(),
                                            left_ptr,
                                            &[neg_right],
                                            "ptr_sub",
                                        )
                                        .unwrap()
                                };
                                result.into()
                            }
                        } else if ty.is_floating() {
                            let res = self
                                .builder
                                .build_float_sub(
                                    left_val.into_float_value(),
                                    right_val.into_float_value(),
                                    "fsub",
                                )
                                .unwrap();
                            self.emit_float_overflow_check(res, expr.span);
                            res.into()
                        } else {
                            let left_int = left_val.into_int_value();
                            let right_int = right_val.into_int_value();
                            if self.is_signed_type(ty) {
                                self.emit_checked_arithmetic(
                                    "llvm.ssub.with.overflow.i32",
                                    left_int,
                                    right_int,
                                    expr.span,
                                )
                            } else {
                                self.builder
                                    .build_int_sub(left_int, right_int, "sub")
                                    .unwrap()
                                    .into()
                            }
                        }
                    }
                    BinaryOp::Mul => {
                        let ty = left.ty.as_ref().unwrap();
                        if ty.is_floating() {
                            let res = self
                                .builder
                                .build_float_mul(
                                    left_val.into_float_value(),
                                    right_val.into_float_value(),
                                    "fmul",
                                )
                                .unwrap();
                            self.emit_float_overflow_check(res, expr.span);
                            res.into()
                        } else {
                            let left_int = left_val.into_int_value();
                            let right_int = right_val.into_int_value();
                            if self.is_signed_type(ty) {
                                self.emit_checked_arithmetic(
                                    "llvm.smul.with.overflow.i32",
                                    left_int,
                                    right_int,
                                    expr.span,
                                )
                            } else {
                                self.builder
                                    .build_int_mul(left_int, right_int, "mul")
                                    .unwrap()
                                    .into()
                            }
                        }
                    }
                    BinaryOp::Div => {
                        let ty = left.ty.as_ref().unwrap();
                        if ty.is_floating() {
                            let res = self
                                .builder
                                .build_float_div(
                                    left_val.into_float_value(),
                                    right_val.into_float_value(),
                                    "fdiv",
                                )
                                .unwrap();
                            self.emit_float_overflow_check(res, expr.span);
                            res.into()
                        } else {
                            let left_int = left_val.into_int_value();
                            let right_int = right_val.into_int_value();
                            let is_signed = self.is_signed_type(ty);
                            self.emit_division_guard(left_int, right_int, is_signed, expr.span);
                            if is_signed {
                                self.builder
                                    .build_int_signed_div(left_int, right_int, "div")
                                    .unwrap()
                                    .into()
                            } else {
                                self.builder
                                    .build_int_unsigned_div(left_int, right_int, "div")
                                    .unwrap()
                                    .into()
                            }
                        }
                    }
                    BinaryOp::Mod => {
                        let left_int = left_val.into_int_value();
                        let right_int = right_val.into_int_value();
                        let ty = left.ty.as_ref().unwrap();
                        let is_signed = self.is_signed_type(ty);
                        self.emit_division_guard(left_int, right_int, is_signed, expr.span);
                        if is_signed {
                            self.builder
                                .build_int_signed_rem(left_int, right_int, "rem")
                                .unwrap()
                                .into()
                        } else {
                            self.builder
                                .build_int_unsigned_rem(left_int, right_int, "rem")
                                .unwrap()
                                .into()
                        }
                    }
                    BinaryOp::Equal => {
                        let result = if left.ty.as_ref().unwrap().is_pointer() {
                            let left_int = self
                                .builder
                                .build_ptr_to_int(
                                    left_val.into_pointer_value(),
                                    self.context.i64_type(),
                                    "ptr_cast",
                                )
                                .unwrap();
                            let right_int = self
                                .builder
                                .build_ptr_to_int(
                                    right_val.into_pointer_value(),
                                    self.context.i64_type(),
                                    "ptr_cast",
                                )
                                .unwrap();
                            self.builder
                                .build_int_compare(IntPredicate::EQ, left_int, right_int, "eq")
                                .unwrap()
                        } else if left.ty.as_ref().unwrap().is_floating() {
                            self.builder
                                .build_float_compare(
                                    inkwell::FloatPredicate::OEQ,
                                    left_val.into_float_value(),
                                    right_val.into_float_value(),
                                    "eq",
                                )
                                .unwrap()
                        } else {
                            let left_promoted =
                                self.promote_to_c_arithmetic(left_val, left.ty.as_ref().unwrap());
                            let right_promoted =
                                self.promote_to_c_arithmetic(right_val, right.ty.as_ref().unwrap());
                            let (left_int, right_int) = self.promote_integers(
                                left_promoted,
                                self.is_signed_type(left.ty.as_ref().unwrap()),
                                right_promoted,
                                self.is_signed_type(right.ty.as_ref().unwrap()),
                            );
                            self.builder
                                .build_int_compare(IntPredicate::EQ, left_int, right_int, "eq")
                                .unwrap()
                        };
                        result.into()
                    }
                    BinaryOp::NotEqual => {
                        let result = if left.ty.as_ref().unwrap().is_pointer() {
                            let left_int = self
                                .builder
                                .build_ptr_to_int(
                                    left_val.into_pointer_value(),
                                    self.context.i64_type(),
                                    "ptr_cast",
                                )
                                .unwrap();
                            let right_int = self
                                .builder
                                .build_ptr_to_int(
                                    right_val.into_pointer_value(),
                                    self.context.i64_type(),
                                    "ptr_cast",
                                )
                                .unwrap();
                            self.builder
                                .build_int_compare(IntPredicate::NE, left_int, right_int, "ne")
                                .unwrap()
                        } else if left.ty.as_ref().unwrap().is_floating() {
                            self.builder
                                .build_float_compare(
                                    inkwell::FloatPredicate::UNE,
                                    left_val.into_float_value(),
                                    right_val.into_float_value(),
                                    "ne",
                                )
                                .unwrap()
                        } else {
                            let left_promoted =
                                self.promote_to_c_arithmetic(left_val, left.ty.as_ref().unwrap());
                            let right_promoted =
                                self.promote_to_c_arithmetic(right_val, right.ty.as_ref().unwrap());
                            let (left_int, right_int) = self.promote_integers(
                                left_promoted,
                                self.is_signed_type(left.ty.as_ref().unwrap()),
                                right_promoted,
                                self.is_signed_type(right.ty.as_ref().unwrap()),
                            );
                            self.builder
                                .build_int_compare(IntPredicate::NE, left_int, right_int, "ne")
                                .unwrap()
                        };
                        result.into()
                    }
                    BinaryOp::Less => {
                        let result = if left.ty.as_ref().unwrap().is_pointer() {
                            let left_int = self
                                .builder
                                .build_ptr_to_int(
                                    left_val.into_pointer_value(),
                                    self.context.i64_type(),
                                    "ptr_cast",
                                )
                                .unwrap();
                            let right_int = self
                                .builder
                                .build_ptr_to_int(
                                    right_val.into_pointer_value(),
                                    self.context.i64_type(),
                                    "ptr_cast",
                                )
                                .unwrap();
                            self.builder
                                .build_int_compare(IntPredicate::ULT, left_int, right_int, "lt")
                                .unwrap()
                        } else if left.ty.as_ref().unwrap().is_floating() {
                            self.builder
                                .build_float_compare(
                                    inkwell::FloatPredicate::OLT,
                                    left_val.into_float_value(),
                                    right_val.into_float_value(),
                                    "lt",
                                )
                                .unwrap()
                        } else {
                            let left_promoted =
                                self.promote_to_c_arithmetic(left_val, left.ty.as_ref().unwrap());
                            let right_promoted =
                                self.promote_to_c_arithmetic(right_val, right.ty.as_ref().unwrap());
                            let (left_int, right_int) = self.promote_integers(
                                left_promoted,
                                self.is_signed_type(left.ty.as_ref().unwrap()),
                                right_promoted,
                                self.is_signed_type(right.ty.as_ref().unwrap()),
                            );
                            let is_unsigned = matches!(
                                left.ty.as_ref().unwrap(),
                                Type::UnsignedInt
                                    | Type::UnsignedChar
                                    | Type::UnsignedShort
                                    | Type::UnsignedLong
                            );
                            let pred = if is_unsigned {
                                IntPredicate::ULT
                            } else {
                                IntPredicate::SLT
                            };
                            self.builder
                                .build_int_compare(pred, left_int, right_int, "lt")
                                .unwrap()
                        };
                        result.into()
                    }
                    BinaryOp::LessEqual => {
                        let result = if left.ty.as_ref().unwrap().is_pointer() {
                            let left_int = self
                                .builder
                                .build_ptr_to_int(
                                    left_val.into_pointer_value(),
                                    self.context.i64_type(),
                                    "ptr_cast",
                                )
                                .unwrap();
                            let right_int = self
                                .builder
                                .build_ptr_to_int(
                                    right_val.into_pointer_value(),
                                    self.context.i64_type(),
                                    "ptr_cast",
                                )
                                .unwrap();
                            self.builder
                                .build_int_compare(IntPredicate::ULE, left_int, right_int, "le")
                                .unwrap()
                        } else if left.ty.as_ref().unwrap().is_floating() {
                            self.builder
                                .build_float_compare(
                                    inkwell::FloatPredicate::OLE,
                                    left_val.into_float_value(),
                                    right_val.into_float_value(),
                                    "le",
                                )
                                .unwrap()
                        } else {
                            let left_promoted =
                                self.promote_to_c_arithmetic(left_val, left.ty.as_ref().unwrap());
                            let right_promoted =
                                self.promote_to_c_arithmetic(right_val, right.ty.as_ref().unwrap());
                            let (left_int, right_int) = self.promote_integers(
                                left_promoted,
                                self.is_signed_type(left.ty.as_ref().unwrap()),
                                right_promoted,
                                self.is_signed_type(right.ty.as_ref().unwrap()),
                            );
                            let is_unsigned = matches!(
                                left.ty.as_ref().unwrap(),
                                Type::UnsignedInt
                                    | Type::UnsignedChar
                                    | Type::UnsignedShort
                                    | Type::UnsignedLong
                            );
                            let pred = if is_unsigned {
                                IntPredicate::ULE
                            } else {
                                IntPredicate::SLE
                            };
                            self.builder
                                .build_int_compare(pred, left_int, right_int, "le")
                                .unwrap()
                        };
                        result.into()
                    }
                    BinaryOp::Greater => {
                        let result = if left.ty.as_ref().unwrap().is_pointer() {
                            let left_int = self
                                .builder
                                .build_ptr_to_int(
                                    left_val.into_pointer_value(),
                                    self.context.i64_type(),
                                    "ptr_cast",
                                )
                                .unwrap();
                            let right_int = self
                                .builder
                                .build_ptr_to_int(
                                    right_val.into_pointer_value(),
                                    self.context.i64_type(),
                                    "ptr_cast",
                                )
                                .unwrap();
                            self.builder
                                .build_int_compare(IntPredicate::UGT, left_int, right_int, "gt")
                                .unwrap()
                        } else if left.ty.as_ref().unwrap().is_floating() {
                            self.builder
                                .build_float_compare(
                                    inkwell::FloatPredicate::OGT,
                                    left_val.into_float_value(),
                                    right_val.into_float_value(),
                                    "gt",
                                )
                                .unwrap()
                        } else {
                            let left_promoted =
                                self.promote_to_c_arithmetic(left_val, left.ty.as_ref().unwrap());
                            let right_promoted =
                                self.promote_to_c_arithmetic(right_val, right.ty.as_ref().unwrap());
                            let (left_int, right_int) = self.promote_integers(
                                left_promoted,
                                self.is_signed_type(left.ty.as_ref().unwrap()),
                                right_promoted,
                                self.is_signed_type(right.ty.as_ref().unwrap()),
                            );
                            let is_unsigned = matches!(
                                left.ty.as_ref().unwrap(),
                                Type::UnsignedInt
                                    | Type::UnsignedChar
                                    | Type::UnsignedShort
                                    | Type::UnsignedLong
                            );
                            let pred = if is_unsigned {
                                IntPredicate::UGT
                            } else {
                                IntPredicate::SGT
                            };
                            self.builder
                                .build_int_compare(pred, left_int, right_int, "gt")
                                .unwrap()
                        };
                        result.into()
                    }
                    BinaryOp::GreaterEqual => {
                        let result = if left.ty.as_ref().unwrap().is_pointer() {
                            let left_int = self
                                .builder
                                .build_ptr_to_int(
                                    left_val.into_pointer_value(),
                                    self.context.i64_type(),
                                    "ptr_cast",
                                )
                                .unwrap();
                            let right_int = self
                                .builder
                                .build_ptr_to_int(
                                    right_val.into_pointer_value(),
                                    self.context.i64_type(),
                                    "ptr_cast",
                                )
                                .unwrap();
                            self.builder
                                .build_int_compare(IntPredicate::UGE, left_int, right_int, "ge")
                                .unwrap()
                        } else if left.ty.as_ref().unwrap().is_floating() {
                            self.builder
                                .build_float_compare(
                                    inkwell::FloatPredicate::OGE,
                                    left_val.into_float_value(),
                                    right_val.into_float_value(),
                                    "ge",
                                )
                                .unwrap()
                        } else {
                            let left_promoted =
                                self.promote_to_c_arithmetic(left_val, left.ty.as_ref().unwrap());
                            let right_promoted =
                                self.promote_to_c_arithmetic(right_val, right.ty.as_ref().unwrap());
                            let (left_int, right_int) = self.promote_integers(
                                left_promoted,
                                self.is_signed_type(left.ty.as_ref().unwrap()),
                                right_promoted,
                                self.is_signed_type(right.ty.as_ref().unwrap()),
                            );
                            let is_unsigned = matches!(
                                left.ty.as_ref().unwrap(),
                                Type::UnsignedInt
                                    | Type::UnsignedChar
                                    | Type::UnsignedShort
                                    | Type::UnsignedLong
                            );
                            let pred = if is_unsigned {
                                IntPredicate::UGE
                            } else {
                                IntPredicate::SGE
                            };
                            self.builder
                                .build_int_compare(pred, left_int, right_int, "ge")
                                .unwrap()
                        };
                        result.into()
                    }
                    BinaryOp::Shl => {
                        let left_int = left_val.into_int_value();
                        let right_int = right_val.into_int_value();
                        let bit_width = left_int.get_type().get_bit_width();
                        let mask = right_int
                            .get_type()
                            .const_int((bit_width - 1) as u64, false);
                        let masked_right_orig = self
                            .builder
                            .build_and(right_int, mask, "masked_shift_orig")
                            .unwrap();

                        let masked_right = if right_int.get_type().get_bit_width() > bit_width {
                            self.builder
                                .build_int_truncate(
                                    masked_right_orig,
                                    left_int.get_type(),
                                    "masked_shift_trunc",
                                )
                                .unwrap()
                        } else if right_int.get_type().get_bit_width() < bit_width {
                            self.builder
                                .build_int_z_extend(
                                    masked_right_orig,
                                    left_int.get_type(),
                                    "masked_shift_extend",
                                )
                                .unwrap()
                        } else {
                            masked_right_orig
                        };

                        self.builder
                            .build_left_shift(left_int, masked_right, "shl")
                            .unwrap()
                            .into()
                    }
                    BinaryOp::Shr => {
                        let left_int = left_val.into_int_value();
                        let right_int = right_val.into_int_value();
                        let bit_width = left_int.get_type().get_bit_width();
                        let mask = right_int
                            .get_type()
                            .const_int((bit_width - 1) as u64, false);
                        let masked_right_orig = self
                            .builder
                            .build_and(right_int, mask, "masked_shift_orig")
                            .unwrap();

                        let masked_right = if right_int.get_type().get_bit_width() > bit_width {
                            self.builder
                                .build_int_truncate(
                                    masked_right_orig,
                                    left_int.get_type(),
                                    "masked_shift_trunc",
                                )
                                .unwrap()
                        } else if right_int.get_type().get_bit_width() < bit_width {
                            self.builder
                                .build_int_z_extend(
                                    masked_right_orig,
                                    left_int.get_type(),
                                    "masked_shift_extend",
                                )
                                .unwrap()
                        } else {
                            masked_right_orig
                        };

                        let is_signed = matches!(
                            left.ty.as_ref().unwrap(),
                            Type::Char | Type::Int | Type::Short | Type::Long
                        );
                        self.builder
                            .build_right_shift(left_int, masked_right, is_signed, "shr")
                            .unwrap()
                            .into()
                    }
                    BinaryOp::BitAnd => {
                        let left_int = left_val.into_int_value();
                        let right_int = right_val.into_int_value();
                        self.builder
                            .build_and(left_int, right_int, "and")
                            .unwrap()
                            .into()
                    }
                    BinaryOp::BitOr => {
                        let left_int = left_val.into_int_value();
                        let right_int = right_val.into_int_value();
                        self.builder
                            .build_or(left_int, right_int, "or")
                            .unwrap()
                            .into()
                    }
                    BinaryOp::BitXor => {
                        let left_int = left_val.into_int_value();
                        let right_int = right_val.into_int_value();
                        self.builder
                            .build_xor(left_int, right_int, "xor")
                            .unwrap()
                            .into()
                    }
                    BinaryOp::LogicalAnd => {
                        let left_bool = if left_val.is_pointer_value() {
                            let int_val = self
                                .builder
                                .build_ptr_to_int(
                                    left_val.into_pointer_value(),
                                    self.context.i64_type(),
                                    "ptr_cast",
                                )
                                .unwrap();
                            self.builder
                                .build_int_compare(
                                    IntPredicate::NE,
                                    int_val,
                                    int_val.get_type().const_zero(),
                                    "not_null",
                                )
                                .unwrap()
                        } else {
                            let int_val = left_val.into_int_value();
                            self.builder
                                .build_int_compare(
                                    IntPredicate::NE,
                                    int_val,
                                    int_val.get_type().const_zero(),
                                    "not_zero",
                                )
                                .unwrap()
                        };

                        let right_bool = if right_val.is_pointer_value() {
                            let int_val = self
                                .builder
                                .build_ptr_to_int(
                                    right_val.into_pointer_value(),
                                    self.context.i64_type(),
                                    "ptr_cast",
                                )
                                .unwrap();
                            self.builder
                                .build_int_compare(
                                    IntPredicate::NE,
                                    int_val,
                                    int_val.get_type().const_zero(),
                                    "not_null",
                                )
                                .unwrap()
                        } else {
                            let int_val = right_val.into_int_value();
                            self.builder
                                .build_int_compare(
                                    IntPredicate::NE,
                                    int_val,
                                    int_val.get_type().const_zero(),
                                    "not_zero",
                                )
                                .unwrap()
                        };

                        let res_i1 = self
                            .builder
                            .build_and(left_bool, right_bool, "land")
                            .unwrap();
                        self.builder
                            .build_int_z_extend(res_i1, self.context.i32_type(), "land_cast")
                            .unwrap()
                            .into()
                    }
                    BinaryOp::LogicalOr => {
                        let left_bool = if left_val.is_pointer_value() {
                            let int_val = self
                                .builder
                                .build_ptr_to_int(
                                    left_val.into_pointer_value(),
                                    self.context.i64_type(),
                                    "ptr_cast",
                                )
                                .unwrap();
                            self.builder
                                .build_int_compare(
                                    IntPredicate::NE,
                                    int_val,
                                    int_val.get_type().const_zero(),
                                    "not_null",
                                )
                                .unwrap()
                        } else {
                            let int_val = left_val.into_int_value();
                            self.builder
                                .build_int_compare(
                                    IntPredicate::NE,
                                    int_val,
                                    int_val.get_type().const_zero(),
                                    "not_zero",
                                )
                                .unwrap()
                        };

                        let right_bool = if right_val.is_pointer_value() {
                            let int_val = self
                                .builder
                                .build_ptr_to_int(
                                    right_val.into_pointer_value(),
                                    self.context.i64_type(),
                                    "ptr_cast",
                                )
                                .unwrap();
                            self.builder
                                .build_int_compare(
                                    IntPredicate::NE,
                                    int_val,
                                    int_val.get_type().const_zero(),
                                    "not_null",
                                )
                                .unwrap()
                        } else {
                            let int_val = right_val.into_int_value();
                            self.builder
                                .build_int_compare(
                                    IntPredicate::NE,
                                    int_val,
                                    int_val.get_type().const_zero(),
                                    "not_zero",
                                )
                                .unwrap()
                        };
                        let res_i1 = self.builder.build_or(left_bool, right_bool, "lor").unwrap();
                        self.builder
                            .build_int_z_extend(res_i1, self.context.i32_type(), "lor_cast")
                            .unwrap()
                            .into()
                    }
                }
            }
            ExprNode::Unary(op, inner) => {
                match op {
                    UnaryOp::PreInc | UnaryOp::PostInc | UnaryOp::PreDec | UnaryOp::PostDec => {
                        let is_inc = matches!(op, UnaryOp::PreInc | UnaryOp::PostInc);
                        let is_pre = matches!(op, UnaryOp::PreInc | UnaryOp::PreDec);
                        let ty = inner.ty.as_ref().unwrap();
                        let lval_ptr = self.gen_lvalue(inner);

                        // Alignment check before load
                        self.emit_alignment_check(lval_ptr, ty, expr.span);

                        let current_val =
                            self.build_load_and_sanitize_bool(ty, lval_ptr, "incdec_load");

                        // Compute new value
                        let new_val: BasicValueEnum<'ctx> = if ty.is_pointer() {
                            let left_ptr = current_val.into_pointer_value();
                            let delta = self.context.i64_type().const_int(1, false);
                            let delta = if is_inc {
                                delta
                            } else {
                                self.builder.build_int_neg(delta, "neg").unwrap()
                            };
                            let result = if let Type::Pointer(elem_ty) = ty {
                                let elem_llvm_ty = self.get_llvm_type(elem_ty);
                                unsafe {
                                    self.builder
                                        .build_gep(elem_llvm_ty, left_ptr, &[delta], "ptr_arith")
                                        .unwrap()
                                }
                            } else {
                                unsafe {
                                    self.builder
                                        .build_gep(
                                            self.context.i8_type(),
                                            left_ptr,
                                            &[delta],
                                            "ptr_arith",
                                        )
                                        .unwrap()
                                }
                            };
                            result.into()
                        } else if ty.is_floating() {
                            let one = self.get_llvm_type(ty).into_float_type().const_float(1.0);
                            let res = if is_inc {
                                self.builder
                                    .build_float_add(current_val.into_float_value(), one, "fadd")
                                    .unwrap()
                            } else {
                                self.builder
                                    .build_float_sub(current_val.into_float_value(), one, "fsub")
                                    .unwrap()
                            };
                            self.emit_float_overflow_check(res, expr.span);
                            res.into()
                        } else {
                            let left_int = current_val.into_int_value();
                            let right_int = left_int.get_type().const_int(1, false);
                            if self.is_signed_type(ty) {
                                let intrinsic_op = if is_inc { "sadd" } else { "ssub" };
                                let intrinsic = self.get_overflow_intrinsic(intrinsic_op, left_int);
                                self.emit_checked_arithmetic(
                                    &intrinsic, left_int, right_int, expr.span,
                                )
                            } else {
                                let res = if is_inc {
                                    self.builder
                                        .build_int_add(left_int, right_int, "add")
                                        .unwrap()
                                } else {
                                    self.builder
                                        .build_int_sub(left_int, right_int, "sub")
                                        .unwrap()
                                };
                                res.into()
                            }
                        };

                        // Store new value back
                        self.builder.build_store(lval_ptr, new_val).unwrap();

                        // Propagate pointer metadata if pointer type
                        if ty.is_pointer() {
                            let (base, size, key) =
                                self.get_expr_pointer_metadata_with_val(inner, Some(current_val));
                            self.store_pointer_metadata(inner, base, size, key);
                        }

                        if is_pre {
                            new_val
                        } else {
                            current_val
                        }
                    }
                    _ => {
                        let val = self.gen_expr(inner);
                        match op {
                            UnaryOp::Neg => self
                                .builder
                                .build_int_neg(val.into_int_value(), "neg")
                                .unwrap()
                                .into(),
                            UnaryOp::Not => {
                                let int_val = if val.is_pointer_value() {
                                    self.builder
                                        .build_ptr_to_int(
                                            val.into_pointer_value(),
                                            self.context.i64_type(),
                                            "ptr_to_int",
                                        )
                                        .unwrap()
                                } else {
                                    val.into_int_value()
                                };
                                let cmp = self
                                    .builder
                                    .build_int_compare(
                                        IntPredicate::EQ,
                                        int_val,
                                        int_val.get_type().const_zero(),
                                        "not",
                                    )
                                    .unwrap();
                                cmp.into()
                            }
                            UnaryOp::Deref => {
                                let ptr_val = val.into_pointer_value();

                                // Shadow bounds check
                                if !self.is_deref_statically_safe(inner) {
                                    let (base, size, key) =
                                        self.get_expr_pointer_metadata_with_val(inner, Some(val));
                                    let access_size = self.get_type_size(expr.ty.as_ref().unwrap());
                                    self.emit_bounds_check(
                                        ptr_val,
                                        base,
                                        size,
                                        key,
                                        access_size,
                                        false,
                                        expr.span,
                                    );
                                }

                                // Alignment check
                                self.emit_alignment_check(
                                    ptr_val,
                                    expr.ty.as_ref().unwrap(),
                                    expr.span,
                                );

                                let loaded = self.build_load_and_sanitize_bool(
                                    expr.ty.as_ref().unwrap(),
                                    ptr_val,
                                    "deref",
                                );

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
                }
            }
            ExprNode::Call(callee, args) => {
                let is_direct = if let ExprNode::Identifier(func_name) = &callee.node {
                    !self.variables.contains_key(func_name)
                } else {
                    false
                };

                if is_direct {
                    let func_name = match &callee.node {
                        ExprNode::Identifier(name) => name,
                        _ => unreachable!(),
                    };
                    let func = self
                        .module
                        .get_function(func_name)
                        .expect("Function not found");
                    let mut compiled_args: Vec<BasicValueEnum> =
                        args.iter().map(|arg| self.gen_expr(arg)).collect();

                    if func_name == "printf" || func_name == "sprintf" || func_name == "printf_s" {
                        let fmt_arg_idx = if func_name == "sprintf" { 1 } else { 0 };
                        if args.len() > fmt_arg_idx {
                            let fmt_val = compiled_args[fmt_arg_idx];
                            let num_args = args.len() - fmt_arg_idx - 1;
                            let i32_type = self.context.i32_type();
                            let count_val = i32_type.const_int(num_args as u64, false);
                            let type_ids_val = if num_args > 0 {
                                let type_ids_arr = self
                                    .builder
                                    .build_array_alloca(i32_type, count_val, "type_ids")
                                    .unwrap();
                                for i in 0..num_args {
                                    let arg_ty = args[fmt_arg_idx + 1 + i].ty.as_ref().unwrap();
                                    let id = if arg_ty.is_integer() {
                                        1
                                    } else if arg_ty.is_floating() {
                                        2
                                    } else if self.is_string_pointer(arg_ty) {
                                        3
                                    } else if arg_ty.is_pointer() {
                                        4
                                    } else {
                                        0
                                    };
                                    let index = i32_type.const_int(i as u64, false);
                                    let element_ptr = unsafe {
                                        self.builder
                                            .build_gep(
                                                i32_type,
                                                type_ids_arr,
                                                &[index],
                                                "element_ptr",
                                            )
                                            .unwrap()
                                    };
                                    self.builder
                                        .build_store(
                                            element_ptr,
                                            i32_type.const_int(id as u64, false),
                                        )
                                        .unwrap();
                                }
                                type_ids_arr.into()
                            } else {
                                self.context
                                    .ptr_type(AddressSpace::default())
                                    .const_null()
                                    .into()
                            };

                            self.builder
                                .build_call(
                                    self.validate_printf_fn,
                                    &[fmt_val.into(), count_val.into(), type_ids_val],
                                    "validate_printf",
                                )
                                .unwrap();
                        }
                    }

                    let is_ffi = if let Some(decl) = self.functions.get(func_name) {
                        decl.body.is_none() && !self.is_std_lib_symbol(func_name)
                    } else {
                        !self.is_std_lib_symbol(func_name)
                    };

                    let mut sandboxed_info = Vec::new();
                    if is_ffi {
                        if let Some(decl) = self.functions.get(func_name).cloned() {
                            let func_val = self.current_fn.unwrap();
                            for i in 0..args.len() {
                                if i < decl.params.len() && decl.params[i].ty.is_pointer() {
                                    let arg_val = compiled_args[i];
                                    let (_base, size, _key) = self
                                        .get_expr_pointer_metadata_with_val(
                                            &args[i],
                                            Some(arg_val),
                                        );

                                    let size_val = size.into_int_value();
                                    let is_infinite = self
                                        .builder
                                        .build_int_compare(
                                            IntPredicate::EQ,
                                            size_val,
                                            self.context.i64_type().const_int(u64::MAX, false),
                                            "is_infinite",
                                        )
                                        .unwrap();

                                    let current_bb = self.builder.get_insert_block().unwrap();
                                    let sandbox_bb =
                                        self.context.append_basic_block(func_val, "sandbox_in");
                                    let merge_bb = self
                                        .context
                                        .append_basic_block(func_val, "sandbox_in_merge");

                                    self.builder
                                        .build_conditional_branch(is_infinite, merge_bb, sandbox_bb)
                                        .unwrap();

                                    self.builder.position_at_end(sandbox_bb);
                                    let cast_arg = self
                                        .builder
                                        .build_pointer_cast(
                                            arg_val.into_pointer_value(),
                                            self.context.ptr_type(AddressSpace::default()),
                                            "cast_arg",
                                        )
                                        .unwrap();
                                    let sandboxed_val = self
                                        .builder
                                        .build_call(
                                            self.ffi_sandbox_in_fn,
                                            &[cast_arg.into(), size_val.into()],
                                            "sandbox_call",
                                        )
                                        .unwrap()
                                        .try_as_basic_value()
                                        .left()
                                        .unwrap();
                                    self.builder.build_unconditional_branch(merge_bb).unwrap();

                                    self.builder.position_at_end(merge_bb);
                                    let phi = self
                                        .builder
                                        .build_phi(
                                            self.context.ptr_type(AddressSpace::default()),
                                            "sandbox_phi",
                                        )
                                        .unwrap();
                                    phi.add_incoming(&[
                                        (&arg_val, current_bb),
                                        (&sandboxed_val, sandbox_bb),
                                    ]);

                                    let final_arg_val = phi.as_basic_value();
                                    compiled_args[i] = final_arg_val;
                                    sandboxed_info.push((arg_val, final_arg_val, size_val));
                                }
                            }
                        }
                    }

                    if is_ffi {
                        self.builder
                            .build_call(self.ffi_enter_fn, &[], "ffi_enter")
                            .unwrap();
                    }

                    let call = self
                        .builder
                        .build_call(
                            func,
                            &compiled_args
                                .iter()
                                .map(|v| (*v).into())
                                .collect::<Vec<_>>(),
                            "call",
                        )
                        .unwrap();

                    if is_ffi {
                        self.builder
                            .build_call(self.ffi_leave_fn, &[], "ffi_leave")
                            .unwrap();
                    }

                    let return_val = match call.try_as_basic_value().left() {
                        Some(val) => val,
                        None => self.context.i32_type().const_zero().into(), // Void fallback
                    };

                    if is_ffi {
                        let func_val = self.current_fn.unwrap();
                        for (orig_ptr, sandboxed_ptr, size_val) in sandboxed_info {
                            let is_sandboxed = self
                                .builder
                                .build_int_compare(
                                    IntPredicate::NE,
                                    orig_ptr.into_pointer_value(),
                                    sandboxed_ptr.into_pointer_value(),
                                    "is_sandboxed",
                                )
                                .unwrap();

                            let cleanup_bb =
                                self.context.append_basic_block(func_val, "sandbox_out");
                            let next_bb = self
                                .context
                                .append_basic_block(func_val, "sandbox_out_next");

                            self.builder
                                .build_conditional_branch(is_sandboxed, cleanup_bb, next_bb)
                                .unwrap();

                            self.builder.position_at_end(cleanup_bb);
                            let orig_cast = self
                                .builder
                                .build_pointer_cast(
                                    orig_ptr.into_pointer_value(),
                                    self.context.ptr_type(AddressSpace::default()),
                                    "orig_cast",
                                )
                                .unwrap();
                            let sandboxed_cast = self
                                .builder
                                .build_pointer_cast(
                                    sandboxed_ptr.into_pointer_value(),
                                    self.context.ptr_type(AddressSpace::default()),
                                    "sandboxed_cast",
                                )
                                .unwrap();

                            self.builder
                                .build_call(
                                    self.ffi_sandbox_out_fn,
                                    &[orig_cast.into(), sandboxed_cast.into(), size_val.into()],
                                    "sandbox_out_call",
                                )
                                .unwrap();
                            self.builder.build_unconditional_branch(next_bb).unwrap();

                            self.builder.position_at_end(next_bb);
                        }
                    }

                    return_val
                } else {
                    let callee_val = self.gen_expr(callee).into_pointer_value();

                    let return_type = expr.ty.as_ref().unwrap();
                    let param_types: Vec<Type> = args
                        .iter()
                        .map(|arg| arg.ty.as_ref().unwrap().clone())
                        .collect();
                    let expected_hash = self.compute_signature_hash(return_type, &param_types);

                    let callee_cast = self
                        .builder
                        .build_pointer_cast(
                            callee_val,
                            self.context.ptr_type(AddressSpace::default()),
                            "callee_cast",
                        )
                        .unwrap();
                    let hash_val = self.context.i64_type().const_int(expected_hash, false);
                    self.builder
                        .build_call(
                            self.cfi_check_fn,
                            &[callee_cast.into(), hash_val.into()],
                            "cfi_check",
                        )
                        .unwrap();

                    let rt_llvm = if *return_type == Type::Void {
                        None
                    } else {
                        Some(self.get_llvm_type(return_type))
                    };
                    let params_llvm: Vec<BasicTypeEnum> =
                        param_types.iter().map(|t| self.get_llvm_type(t)).collect();
                    let fn_type = match rt_llvm {
                        Some(rt) => rt.fn_type(
                            &params_llvm.iter().map(|t| (*t).into()).collect::<Vec<_>>(),
                            false,
                        ),
                        None => self.context.void_type().fn_type(
                            &params_llvm.iter().map(|t| (*t).into()).collect::<Vec<_>>(),
                            false,
                        ),
                    };

                    let compiled_args: Vec<BasicValueEnum> =
                        args.iter().map(|arg| self.gen_expr(arg)).collect();
                    let call = self
                        .builder
                        .build_indirect_call(
                            fn_type,
                            callee_val,
                            &compiled_args
                                .iter()
                                .map(|v| (*v).into())
                                .collect::<Vec<_>>(),
                            "call",
                        )
                        .unwrap();
                    match call.try_as_basic_value().left() {
                        Some(val) => val,
                        None => self.context.i32_type().const_zero().into(), // Void fallback
                    }
                }
            }
            ExprNode::Cast(cast_ty, inner) => {
                let val = self.gen_expr(inner);
                if cast_ty.is_pointer() && inner.ty.as_ref().unwrap().is_pointer() {
                    let ptr_val = val.into_pointer_value();
                    let target_llvm_ty = self.get_llvm_type(cast_ty);
                    self.builder
                        .build_pointer_cast(ptr_val, target_llvm_ty.into_pointer_type(), "cast")
                        .unwrap()
                        .into()
                } else if cast_ty.is_integer() && inner.ty.as_ref().unwrap().is_pointer() {
                    let ptr_val = val.into_pointer_value();
                    let target_llvm_ty = self.get_llvm_type(cast_ty);
                    self.builder
                        .build_ptr_to_int(ptr_val, target_llvm_ty.into_int_type(), "cast")
                        .unwrap()
                        .into()
                } else if cast_ty.is_pointer() && inner.ty.as_ref().unwrap().is_integer() {
                    let int_val = val.into_int_value();
                    let target_llvm_ty = self.get_llvm_type(cast_ty);
                    self.builder
                        .build_int_to_ptr(int_val, target_llvm_ty.into_pointer_type(), "cast")
                        .unwrap()
                        .into()
                } else if cast_ty.is_integer() && inner.ty.as_ref().unwrap().is_integer() {
                    let int_val = val.into_int_value();
                    let target_llvm_ty = self.get_llvm_type(cast_ty).into_int_type();
                    let src_width = int_val.get_type().get_bit_width();
                    let dest_width = target_llvm_ty.get_bit_width();
                    if dest_width < src_width {
                        self.builder
                            .build_int_truncate(int_val, target_llvm_ty, "cast")
                            .unwrap()
                            .into()
                    } else if dest_width > src_width {
                        if self.is_signed_type(inner.ty.as_ref().unwrap()) {
                            self.builder
                                .build_int_s_extend(int_val, target_llvm_ty, "cast")
                                .unwrap()
                                .into()
                        } else {
                            self.builder
                                .build_int_z_extend(int_val, target_llvm_ty, "cast")
                                .unwrap()
                                .into()
                        }
                    } else {
                        val
                    }
                } else if cast_ty.is_integer() && inner.ty.as_ref().unwrap().is_floating() {
                    let float_val = val.into_float_value();
                    let target_llvm_ty = self.get_llvm_type(cast_ty).into_int_type();
                    let is_signed = self.is_signed_type(cast_ty);
                    self.emit_float_to_int_guard(float_val, target_llvm_ty, is_signed, expr.span);
                    if is_signed {
                        self.builder
                            .build_float_to_signed_int(float_val, target_llvm_ty, "cast")
                            .unwrap()
                            .into()
                    } else {
                        self.builder
                            .build_float_to_unsigned_int(float_val, target_llvm_ty, "cast")
                            .unwrap()
                            .into()
                    }
                } else if cast_ty.is_floating() && inner.ty.as_ref().unwrap().is_integer() {
                    let int_val = val.into_int_value();
                    let target_llvm_ty = self.get_llvm_type(cast_ty).into_float_type();
                    if self.is_signed_type(inner.ty.as_ref().unwrap()) {
                        self.builder
                            .build_signed_int_to_float(int_val, target_llvm_ty, "cast")
                            .unwrap()
                            .into()
                    } else {
                        self.builder
                            .build_unsigned_int_to_float(int_val, target_llvm_ty, "cast")
                            .unwrap()
                            .into()
                    }
                } else if cast_ty.is_floating() && inner.ty.as_ref().unwrap().is_floating() {
                    let float_val = val.into_float_value();
                    let target_llvm_ty = self.get_llvm_type(cast_ty).into_float_type();
                    let src_width = if float_val.get_type() == self.context.f64_type() {
                        64
                    } else {
                        32
                    };
                    let dest_width = if target_llvm_ty == self.context.f64_type() {
                        64
                    } else {
                        32
                    };
                    if dest_width < src_width {
                        self.builder
                            .build_float_trunc(float_val, target_llvm_ty, "cast")
                            .unwrap()
                            .into()
                    } else if dest_width > src_width {
                        self.builder
                            .build_float_ext(float_val, target_llvm_ty, "cast")
                            .unwrap()
                            .into()
                    } else {
                        val
                    }
                } else {
                    val
                }
            }
            ExprNode::Member(inner, member_name, is_arrow) => {
                let member_ptr = self.gen_member_pointer(inner, member_name, *is_arrow);
                let member_ty = expr.ty.as_ref().unwrap();
                let _llvm_ty = self.get_llvm_type(member_ty);

                // Alignment check
                self.emit_alignment_check(member_ptr, member_ty, expr.span);

                // Load the member value
                self.build_load_and_sanitize_bool(member_ty, member_ptr, member_name)
            }
            ExprNode::SizeofExpr(inner) => {
                let inner_ty = inner.ty.as_ref().unwrap();
                self.get_type_size(inner_ty)
            }
            ExprNode::SizeofType(target_ty) => self.get_type_size(target_ty),
            ExprNode::AlignofExpr(inner) => {
                let inner_ty = inner.ty.as_ref().unwrap();
                let (_, align) = self.get_type_size_and_align(inner_ty);
                self.context
                    .i64_type()
                    .const_int(align as u64, false)
                    .into()
            }
            ExprNode::AlignofType(target_ty) => {
                let (_, align) = self.get_type_size_and_align(target_ty);
                self.context
                    .i64_type()
                    .const_int(align as u64, false)
                    .into()
            }
        }
    }

    fn get_type_size(&self, ty: &Type) -> BasicValueEnum<'ctx> {
        let (size, _) = self.get_type_size_and_align(ty);
        self.context.i64_type().const_int(size as u64, false).into()
    }

    fn load_shadow_metadata(
        &mut self,
        ptr_val: PointerValue<'ctx>,
    ) -> (
        BasicValueEnum<'ctx>,
        BasicValueEnum<'ctx>,
        BasicValueEnum<'ctx>,
    ) {
        let base_out = self.create_entry_block_alloca(
            self.context.ptr_type(AddressSpace::default()),
            "shadow_base",
        );
        let size_out = self.create_entry_block_alloca(self.context.i64_type(), "shadow_size");
        let key_out = self.create_entry_block_alloca(self.context.i64_type(), "shadow_key");

        let ptr_addr_cast = self
            .builder
            .build_pointer_cast(
                ptr_val,
                self.context.ptr_type(AddressSpace::default()),
                "ptr_cast",
            )
            .unwrap();
        self.builder
            .build_call(
                self.shadow_load_fn,
                &[
                    ptr_addr_cast.into(),
                    base_out.into(),
                    size_out.into(),
                    key_out.into(),
                ],
                "shadow_load",
            )
            .unwrap();

        let base = self
            .builder
            .build_load(
                self.context.ptr_type(AddressSpace::default()),
                base_out,
                "base",
            )
            .unwrap();
        let size = self
            .builder
            .build_load(self.context.i64_type(), size_out, "size")
            .unwrap();
        let key = self
            .builder
            .build_load(self.context.i64_type(), key_out, "key")
            .unwrap();
        (base, size, key)
    }

    fn get_expr_pointer_metadata_with_val(
        &mut self,
        expr: &Expr,
        val: Option<BasicValueEnum<'ctx>>,
    ) -> (
        BasicValueEnum<'ctx>,
        BasicValueEnum<'ctx>,
        BasicValueEnum<'ctx>,
    ) {
        match &expr.node {
            ExprNode::Identifier(name) => {
                if let Some((base_alloca, size_alloca, key_alloca)) =
                    self.pointer_metadata.get(name)
                {
                    let base = self
                        .builder
                        .build_load(
                            self.context.ptr_type(AddressSpace::default()),
                            *base_alloca,
                            "base",
                        )
                        .unwrap();
                    let size = self
                        .builder
                        .build_load(self.context.i64_type(), *size_alloca, "size")
                        .unwrap();
                    let key = self
                        .builder
                        .build_load(self.context.i64_type(), *key_alloca, "key")
                        .unwrap();
                    (base, size, key)
                } else {
                    // Global variable, which is infinite bounds
                    let null_base = self
                        .context
                        .ptr_type(AddressSpace::default())
                        .const_null()
                        .into();
                    let infinite_size = self.context.i64_type().const_int(u64::MAX, false).into();
                    let wildcard_key = self.context.i64_type().const_int(0, false).into();
                    (null_base, infinite_size, wildcard_key)
                }
            }
            ExprNode::Literal(Literal::Nullptr) => {
                let null_base = self
                    .context
                    .ptr_type(AddressSpace::default())
                    .const_null()
                    .into();
                let zero = self.context.i64_type().const_zero().into();
                (null_base, zero, zero)
            }
            ExprNode::Literal(Literal::String(v)) => {
                let string_val = val
                    .unwrap_or_else(|| self.gen_expr(expr))
                    .into_pointer_value();
                let len = v.len() + 1; // including null terminator
                let size = self.context.i64_type().const_int(len as u64, false).into();
                // Set the highest bit of key to 1 (const)
                let const_key = self.context.i64_type().const_int(1 << 63, false).into();
                (string_val.into(), size, const_key)
            }
            ExprNode::Unary(UnaryOp::AddrOf, inner) => {
                let addr = val.unwrap_or_else(|| self.gen_expr(expr));
                let inner_ty = inner.ty.as_ref().unwrap();
                let size = self.get_type_size(inner_ty);
                let key = if let ExprNode::Identifier(name) = &inner.node {
                    if let Some(key_slot) = self.variable_keys.get(name) {
                        let loaded_key = self
                            .builder
                            .build_load(self.context.i64_type(), *key_slot, "loaded_key")
                            .unwrap()
                            .into_int_value();
                        if Self::has_const(inner_ty) {
                            let const_mask = self.context.i64_type().const_int(1 << 63, false);
                            self.builder
                                .build_or(loaded_key, const_mask, "const_key")
                                .unwrap()
                                .into()
                        } else {
                            loaded_key.into()
                        }
                    } else if Self::has_const(inner_ty) {
                        self.context.i64_type().const_int(1 << 63, false).into()
                    } else {
                        self.context.i64_type().const_zero().into()
                    }
                } else if Self::has_const(inner_ty) {
                    self.context.i64_type().const_int(1 << 63, false).into()
                } else {
                    self.context.i64_type().const_zero().into()
                };
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
            ExprNode::Cast(_, inner) => self.get_expr_pointer_metadata_with_val(inner, val),
            ExprNode::Call(_, _) => {
                let ptr_val = val
                    .unwrap_or_else(|| self.gen_expr(expr))
                    .into_pointer_value();
                self.load_shadow_metadata(ptr_val)
            }
            ExprNode::Member(inner, member_name, is_arrow) => {
                let member_ptr = self.gen_member_pointer(inner, member_name, *is_arrow);
                self.load_shadow_metadata(member_ptr)
            }
            _ => {
                // Wildcard fallback
                let null_base = self
                    .context
                    .ptr_type(AddressSpace::default())
                    .const_null()
                    .into();
                let infinite_size = self.context.i64_type().const_int(u64::MAX, false).into();
                let wildcard_key = self.context.i64_type().const_int(0, false).into();
                (null_base, infinite_size, wildcard_key)
            }
        }
    }

    #[allow(dead_code)]
    fn get_expr_pointer_metadata(
        &mut self,
        expr: &Expr,
    ) -> (
        BasicValueEnum<'ctx>,
        BasicValueEnum<'ctx>,
        BasicValueEnum<'ctx>,
    ) {
        self.get_expr_pointer_metadata_with_val(expr, None)
    }

    #[allow(clippy::too_many_arguments)]
    fn emit_bounds_check(
        &mut self,
        ptr: PointerValue<'ctx>,
        base: BasicValueEnum<'ctx>,
        size: BasicValueEnum<'ctx>,
        key: BasicValueEnum<'ctx>,
        access_size: BasicValueEnum<'ctx>,
        is_write: bool,
        _span: Span,
    ) {
        let ptr_cast = self
            .builder
            .build_pointer_cast(
                ptr,
                self.context.ptr_type(AddressSpace::default()),
                "ptr_cast",
            )
            .unwrap();
        let base_cast = self
            .builder
            .build_pointer_cast(
                base.into_pointer_value(),
                self.context.ptr_type(AddressSpace::default()),
                "base_cast",
            )
            .unwrap();

        let file_val = self
            .builder
            .build_global_string_ptr(&self.filename, "file_name")
            .unwrap();
        let line_val = self.context.i32_type().const_int(0, false);
        let is_write_val = self
            .context
            .bool_type()
            .const_int(if is_write { 1 } else { 0 }, false);

        self.builder
            .build_call(
                self.check_bounds_fn,
                &[
                    ptr_cast.into(),
                    base_cast.into(),
                    size.into(),
                    key.into(),
                    access_size.into(),
                    is_write_val.into(),
                    file_val.as_pointer_value().into(),
                    line_val.into(),
                ],
                "bounds_check",
            )
            .unwrap();
    }

    fn emit_division_guard(
        &mut self,
        dividend: IntValue<'ctx>,
        divisor: IntValue<'ctx>,
        is_signed: bool,
        _span: Span,
    ) {
        let func = self.current_fn.unwrap();
        let is_zero = self
            .builder
            .build_int_compare(
                IntPredicate::EQ,
                divisor,
                divisor.get_type().const_zero(),
                "is_zero",
            )
            .unwrap();

        if !is_signed {
            let abort_zero_bb = self.context.append_basic_block(func, "div_zero_abort");
            let cont_bb = self.context.append_basic_block(func, "div_cont");

            self.builder
                .build_conditional_branch(is_zero, abort_zero_bb, cont_bb)
                .unwrap();

            // Abort Zero block
            self.builder.position_at_end(abort_zero_bb);
            let msg_zero = self
                .builder
                .build_global_string_ptr("Division by zero", "msg_zero")
                .unwrap();
            let file = self
                .builder
                .build_global_string_ptr(&self.filename, "file")
                .unwrap();
            let line = self.context.i32_type().const_int(0, false);
            self.builder
                .build_call(
                    self.abort_fn,
                    &[
                        msg_zero.as_pointer_value().into(),
                        file.as_pointer_value().into(),
                        line.into(),
                    ],
                    "abort",
                )
                .unwrap();
            self.builder.build_unreachable().unwrap();

            // Continue block
            self.builder.position_at_end(cont_bb);
            return;
        }

        let width = divisor.get_type().get_bit_width();
        let int_min_val = if width == 64 {
            i64::MIN as u64
        } else if width == 32 {
            i32::MIN as i64 as u64
        } else if width == 16 {
            i16::MIN as i64 as u64
        } else {
            i8::MIN as i64 as u64
        };
        let int_min = divisor.get_type().const_int(int_min_val, false);
        let minus_one = divisor.get_type().const_all_ones();

        let is_int_min = self
            .builder
            .build_int_compare(IntPredicate::EQ, dividend, int_min, "is_int_min")
            .unwrap();
        let is_minus_one = self
            .builder
            .build_int_compare(IntPredicate::EQ, divisor, minus_one, "is_minus_one")
            .unwrap();
        let is_overflow = self
            .builder
            .build_and(is_int_min, is_minus_one, "is_overflow")
            .unwrap();

        let abort_zero_bb = self.context.append_basic_block(func, "div_zero_abort");
        let check_overflow_bb = self.context.append_basic_block(func, "check_overflow");
        let abort_overflow_bb = self.context.append_basic_block(func, "div_overflow_abort");
        let cont_bb = self.context.append_basic_block(func, "div_cont");

        self.builder
            .build_conditional_branch(is_zero, abort_zero_bb, check_overflow_bb)
            .unwrap();

        // Check Overflow Block
        self.builder.position_at_end(check_overflow_bb);
        self.builder
            .build_conditional_branch(is_overflow, abort_overflow_bb, cont_bb)
            .unwrap();

        // Abort Zero block
        self.builder.position_at_end(abort_zero_bb);
        let msg_zero = self
            .builder
            .build_global_string_ptr("Division by zero", "msg_zero")
            .unwrap();
        let file = self
            .builder
            .build_global_string_ptr(&self.filename, "file")
            .unwrap();
        let line = self.context.i32_type().const_int(0, false);
        self.builder
            .build_call(
                self.abort_fn,
                &[
                    msg_zero.as_pointer_value().into(),
                    file.as_pointer_value().into(),
                    line.into(),
                ],
                "abort",
            )
            .unwrap();
        self.builder.build_unreachable().unwrap();

        // Abort Overflow block
        self.builder.position_at_end(abort_overflow_bb);
        let msg_ovf = self
            .builder
            .build_global_string_ptr("Division overflow", "msg_ovf")
            .unwrap();
        let file = self
            .builder
            .build_global_string_ptr(&self.filename, "file")
            .unwrap();
        let line = self.context.i32_type().const_int(0, false);
        self.builder
            .build_call(
                self.abort_fn,
                &[
                    msg_ovf.as_pointer_value().into(),
                    file.as_pointer_value().into(),
                    line.into(),
                ],
                "abort",
            )
            .unwrap();
        self.builder.build_unreachable().unwrap();

        // Continue block
        self.builder.position_at_end(cont_bb);
    }

    fn emit_checked_arithmetic(
        &mut self,
        intrinsic_name: &str,
        left: IntValue<'ctx>,
        right: IntValue<'ctx>,
        _span: Span,
    ) -> BasicValueEnum<'ctx> {
        let func = self.current_fn.unwrap();

        // Define struct { i32, i1 } returned by intrinsics
        let struct_ty = self.context.struct_type(
            &[left.get_type().into(), self.context.bool_type().into()],
            false,
        );
        let intrinsic_fn = self.module.add_function(
            intrinsic_name,
            struct_ty.fn_type(&[left.get_type().into(), right.get_type().into()], false),
            None,
        );

        let result_struct = self
            .builder
            .build_call(intrinsic_fn, &[left.into(), right.into()], "arith_val")
            .unwrap()
            .try_as_basic_value()
            .left()
            .unwrap()
            .into_struct_value();

        let val = self
            .builder
            .build_extract_value(result_struct, 0, "res")
            .unwrap();
        let overflow = self
            .builder
            .build_extract_value(result_struct, 1, "ovf")
            .unwrap()
            .into_int_value();

        let abort_bb = self.context.append_basic_block(func, "overflow_abort");
        let cont_bb = self.context.append_basic_block(func, "overflow_cont");

        self.builder
            .build_conditional_branch(overflow, abort_bb, cont_bb)
            .unwrap();

        // Abort block
        self.builder.position_at_end(abort_bb);
        let msg = self
            .builder
            .build_global_string_ptr("Integer overflow detected", "msg")
            .unwrap();
        let file = self
            .builder
            .build_global_string_ptr(&self.filename, "file")
            .unwrap();
        let line = self.context.i32_type().const_int(0, false);
        self.builder
            .build_call(
                self.abort_fn,
                &[
                    msg.as_pointer_value().into(),
                    file.as_pointer_value().into(),
                    line.into(),
                ],
                "abort",
            )
            .unwrap();
        self.builder.build_unreachable().unwrap();

        // Continue block
        self.builder.position_at_end(cont_bb);

        val
    }

    fn is_signed_type(&self, ty: &Type) -> bool {
        matches!(ty, Type::Char | Type::Int | Type::Short | Type::Long)
    }

    fn promote_to_c_arithmetic(&self, val: BasicValueEnum<'ctx>, ty: &Type) -> IntValue<'ctx> {
        let int_val = val.into_int_value();
        let width = int_val.get_type().get_bit_width();
        let signed = self.is_signed_type(ty);

        if width < 32 {
            if signed {
                self.builder
                    .build_int_s_extend(int_val, self.context.i32_type(), "promote_32")
                    .unwrap()
            } else {
                self.builder
                    .build_int_z_extend(int_val, self.context.i32_type(), "promote_32")
                    .unwrap()
            }
        } else {
            int_val
        }
    }

    fn promote_integers(
        &self,
        left: IntValue<'ctx>,
        left_signed: bool,
        right: IntValue<'ctx>,
        right_signed: bool,
    ) -> (IntValue<'ctx>, IntValue<'ctx>) {
        let left_width = left.get_type().get_bit_width();
        let right_width = right.get_type().get_bit_width();

        if left_width < right_width {
            let promoted_left = if left_signed {
                self.builder
                    .build_int_s_extend(left, right.get_type(), "promote_left")
                    .unwrap()
            } else {
                self.builder
                    .build_int_z_extend(left, right.get_type(), "promote_left")
                    .unwrap()
            };
            (promoted_left, right)
        } else if left_width > right_width {
            let promoted_right = if right_signed {
                self.builder
                    .build_int_s_extend(right, left.get_type(), "promote_right")
                    .unwrap()
            } else {
                self.builder
                    .build_int_z_extend(right, left.get_type(), "promote_right")
                    .unwrap()
            };
            (left, promoted_right)
        } else {
            (left, right)
        }
    }

    fn get_type_size_and_align(&self, ty: &Type) -> (usize, usize) {
        match ty {
            Type::Void => (0, 1),
            Type::Bool | Type::Char | Type::UnsignedChar => (1, 1),
            Type::Short | Type::UnsignedShort => (2, 2),
            Type::Int | Type::UnsignedInt | Type::Float => (4, 4),
            Type::Long | Type::UnsignedLong | Type::Double | Type::Pointer(_) | Type::Nullptr => {
                (8, 8)
            }
            Type::Array(inner, size) => {
                let (sz, al) = self.get_type_size_and_align(inner);
                match size {
                    ArraySize::Const(len) => (sz * len, al),
                    ArraySize::Variable(_) => (8, 8),
                }
            }
            Type::Struct(name) => {
                if let Some(decl) = self.structs.get(name) {
                    let mut current_offset = 0;
                    let mut max_align = 1;
                    for field in &decl.fields {
                        let (f_sz, f_al) = self.get_type_size_and_align(&field.ty);
                        max_align = std::cmp::max(max_align, f_al);
                        current_offset = (current_offset + f_al - 1) & !(f_al - 1);
                        current_offset += f_sz;
                    }
                    let total_size = (current_offset + max_align - 1) & !(max_align - 1);
                    (total_size, max_align)
                } else {
                    (8, 8)
                }
            }
            Type::Union(name) => {
                if let Some(decl) = self.unions.get(name) {
                    let mut max_size = 0;
                    let mut max_align = 1;
                    for field in &decl.fields {
                        let (f_sz, f_al) = self.get_type_size_and_align(&field.ty);
                        max_size = std::cmp::max(max_size, f_sz);
                        max_align = std::cmp::max(max_align, f_al);
                    }
                    let total_size = (max_size + max_align - 1) & !(max_align - 1);
                    (total_size, max_align)
                } else {
                    (8, 8)
                }
            }
            Type::Enum(_) => (4, 4),
            Type::Const(inner) => self.get_type_size_and_align(inner),
            Type::Atomic(inner) => self.get_type_size_and_align(inner),
            _ => (8, 8),
        }
    }

    fn emit_float_to_int_guard(
        &mut self,
        float_val: FloatValue<'ctx>,
        target_int_ty: IntType<'ctx>,
        is_signed: bool,
        _span: Span,
    ) {
        let func = self.current_fn.unwrap();
        let width = target_int_ty.get_bit_width();
        let (min_val, max_val) = if is_signed {
            if width == 64 {
                (i64::MIN as f64 - 1.0, i64::MAX as f64 + 1.0)
            } else if width == 32 {
                (i32::MIN as f64 - 1.0, i32::MAX as f64 + 1.0)
            } else if width == 16 {
                (i16::MIN as f64 - 1.0, i16::MAX as f64 + 1.0)
            } else {
                (i8::MIN as f64 - 1.0, i8::MAX as f64 + 1.0)
            }
        } else if width == 64 {
            (-1.0, u64::MAX as f64 + 1.0)
        } else if width == 32 {
            (-1.0, u32::MAX as f64 + 1.0)
        } else if width == 16 {
            (-1.0, u16::MAX as f64 + 1.0)
        } else {
            (-1.0, u8::MAX as f64 + 1.0)
        };

        let double_ty = self.context.f64_type();
        let f64_val = if float_val.get_type() != double_ty {
            self.builder
                .build_float_ext(float_val, double_ty, "float_ext")
                .unwrap()
        } else {
            float_val
        };

        let min_bound = double_ty.const_float(min_val);
        let max_bound = double_ty.const_float(max_val);

        let is_too_low = self
            .builder
            .build_float_compare(inkwell::FloatPredicate::OLE, f64_val, min_bound, "too_low")
            .unwrap();
        let is_too_high = self
            .builder
            .build_float_compare(inkwell::FloatPredicate::OGE, f64_val, max_bound, "too_high")
            .unwrap();
        let is_overflow = self
            .builder
            .build_or(is_too_low, is_too_high, "is_overflow")
            .unwrap();

        let abort_bb = self.context.append_basic_block(func, "float_to_int_abort");
        let cont_bb = self.context.append_basic_block(func, "float_to_int_cont");

        self.builder
            .build_conditional_branch(is_overflow, abort_bb, cont_bb)
            .unwrap();

        self.builder.position_at_end(abort_bb);
        let msg = self
            .builder
            .build_global_string_ptr("Float-to-int conversion overflow", "msg")
            .unwrap();
        let file = self
            .builder
            .build_global_string_ptr(&self.filename, "file")
            .unwrap();
        let line = self.context.i32_type().const_int(0, false);
        self.builder
            .build_call(
                self.abort_fn,
                &[
                    msg.as_pointer_value().into(),
                    file.as_pointer_value().into(),
                    line.into(),
                ],
                "abort",
            )
            .unwrap();
        self.builder.build_unreachable().unwrap();

        self.builder.position_at_end(cont_bb);
    }

    fn build_load_and_sanitize_bool(
        &self,
        ty: &Type,
        ptr: PointerValue<'ctx>,
        name: &str,
    ) -> BasicValueEnum<'ctx> {
        let llvm_ty = self.get_llvm_type(ty);
        if ty.is_bool() {
            let i8_ptr = self
                .builder
                .build_pointer_cast(
                    ptr,
                    self.context.ptr_type(AddressSpace::default()),
                    &format!("{name}_i8_ptr"),
                )
                .unwrap();
            let i8_val = self
                .builder
                .build_load(self.context.i8_type(), i8_ptr, &format!("{name}_i8_val"))
                .unwrap()
                .into_int_value();
            let cmp = self
                .builder
                .build_int_compare(
                    IntPredicate::NE,
                    i8_val,
                    self.context.i8_type().const_zero(),
                    &format!("{name}_sanitize"),
                )
                .unwrap();
            cmp.into()
        } else {
            self.builder.build_load(llvm_ty, ptr, name).unwrap()
        }
    }

    fn get_overflow_intrinsic(&self, op_name: &str, int_val: IntValue<'ctx>) -> String {
        let bit_width = int_val.get_type().get_bit_width();
        format!("llvm.{op_name}.with.overflow.i{bit_width}")
    }

    fn store_pointer_metadata(
        &mut self,
        lval: &Expr,
        base_val: BasicValueEnum<'ctx>,
        size_val: BasicValueEnum<'ctx>,
        key_val: BasicValueEnum<'ctx>,
    ) {
        match &lval.node {
            ExprNode::Identifier(name) => {
                if let Some((base_alloca, size_alloca, key_alloca)) =
                    self.pointer_metadata.get(name)
                {
                    self.builder.build_store(*base_alloca, base_val).unwrap();
                    self.builder.build_store(*size_alloca, size_val).unwrap();
                    self.builder.build_store(*key_alloca, key_val).unwrap();
                }
            }
            ExprNode::Unary(UnaryOp::Deref, inner) => {
                let dest_ptr = self.gen_expr(inner).into_pointer_value();
                let ptr_addr_cast = self
                    .builder
                    .build_pointer_cast(
                        dest_ptr,
                        self.context.ptr_type(AddressSpace::default()),
                        "ptr_cast",
                    )
                    .unwrap();
                let base_cast = self
                    .builder
                    .build_pointer_cast(
                        base_val.into_pointer_value(),
                        self.context.ptr_type(AddressSpace::default()),
                        "base_cast",
                    )
                    .unwrap();
                self.builder
                    .build_call(
                        self.shadow_store_fn,
                        &[
                            ptr_addr_cast.into(),
                            base_cast.into(),
                            size_val.into(),
                            key_val.into(),
                        ],
                        "shadow_store",
                    )
                    .unwrap();
            }
            ExprNode::Member(inner, member_name, is_arrow) => {
                let member_ptr = self.gen_member_pointer(inner, member_name, *is_arrow);
                let ptr_addr_cast = self
                    .builder
                    .build_pointer_cast(
                        member_ptr,
                        self.context.ptr_type(AddressSpace::default()),
                        "ptr_cast",
                    )
                    .unwrap();
                let base_cast = self
                    .builder
                    .build_pointer_cast(
                        base_val.into_pointer_value(),
                        self.context.ptr_type(AddressSpace::default()),
                        "base_cast",
                    )
                    .unwrap();
                self.builder
                    .build_call(
                        self.shadow_store_fn,
                        &[
                            ptr_addr_cast.into(),
                            base_cast.into(),
                            size_val.into(),
                            key_val.into(),
                        ],
                        "shadow_store",
                    )
                    .unwrap();
            }
            _ => {}
        }
    }

    fn emit_alignment_check(&mut self, ptr: PointerValue<'ctx>, ty: &Type, _span: Span) {
        let ty = match ty {
            Type::Const(inner) => inner,
            _ => ty,
        };
        let align = match ty {
            Type::Char | Type::UnsignedChar => 1,
            Type::Short | Type::UnsignedShort => 2,
            Type::Int | Type::UnsignedInt | Type::Float => 4,
            Type::Long | Type::UnsignedLong | Type::Double | Type::Pointer(_) | Type::Nullptr => 8,
            Type::Struct(name) => self.get_type_size_and_align(&Type::Struct(name.clone())).1,
            Type::Union(name) => self.get_type_size_and_align(&Type::Union(name.clone())).1,
            _ => 1,
        };
        if align <= 1 {
            return;
        }

        let func = self.current_fn.unwrap();
        let ptr_addr = self
            .builder
            .build_ptr_to_int(ptr, self.context.i64_type(), "ptr_addr")
            .unwrap();
        let align_mask = self.context.i64_type().const_int((align - 1) as u64, false);
        let rem = self
            .builder
            .build_and(ptr_addr, align_mask, "align_rem")
            .unwrap();

        let is_misaligned = self
            .builder
            .build_int_compare(
                IntPredicate::NE,
                rem,
                self.context.i64_type().const_zero(),
                "is_misaligned",
            )
            .unwrap();

        let abort_bb = self.context.append_basic_block(func, "align_abort");
        let cont_bb = self.context.append_basic_block(func, "align_cont");

        self.builder
            .build_conditional_branch(is_misaligned, abort_bb, cont_bb)
            .unwrap();

        self.builder.position_at_end(abort_bb);
        let msg = self
            .builder
            .build_global_string_ptr("Unaligned memory access", "msg")
            .unwrap();
        let file = self
            .builder
            .build_global_string_ptr(&self.filename, "file")
            .unwrap();
        let line = self.context.i32_type().const_int(0, false);
        self.builder
            .build_call(
                self.abort_fn,
                &[
                    msg.as_pointer_value().into(),
                    file.as_pointer_value().into(),
                    line.into(),
                ],
                "abort",
            )
            .unwrap();
        self.builder.build_unreachable().unwrap();

        self.builder.position_at_end(cont_bb);
    }

    fn gen_lvalue(&mut self, expr: &Expr) -> PointerValue<'ctx> {
        match &expr.node {
            ExprNode::Identifier(name) => *self.variables.get(name).unwrap(),
            ExprNode::Unary(UnaryOp::Deref, inner) => self.gen_expr(inner).into_pointer_value(),
            ExprNode::Member(inner, member_name, is_arrow) => {
                self.gen_member_pointer(inner, member_name, *is_arrow)
            }
            _ => panic!("Not an lvalue: {expr:?}"),
        }
    }

    fn gen_member_pointer(
        &mut self,
        inner: &Expr,
        member_name: &str,
        is_arrow: bool,
    ) -> PointerValue<'ctx> {
        let base_ptr = if is_arrow {
            self.gen_expr(inner).into_pointer_value()
        } else {
            self.gen_lvalue(inner)
        };

        let inner_ty = inner.ty.as_ref().unwrap();
        let struct_or_union_ty = if is_arrow {
            if let Type::Pointer(base) = inner_ty {
                &**base
            } else {
                inner_ty
            }
        } else {
            inner_ty
        };

        let struct_or_union_ty = match struct_or_union_ty {
            Type::Const(t) => &**t,
            _ => struct_or_union_ty,
        };

        match struct_or_union_ty {
            Type::Struct(name) => {
                let struct_type = self.struct_types.get(name).unwrap();
                let decl = self.structs.get(name).cloned().unwrap();
                let field_idx = decl
                    .fields
                    .iter()
                    .position(|f| f.name == member_name)
                    .unwrap();
                self.builder
                    .build_struct_gep(*struct_type, base_ptr, field_idx as u32, "struct_gep")
                    .unwrap()
            }
            Type::Union(name) => {
                let decl = self.unions.get(name).unwrap();
                let _field = decl.fields.iter().find(|f| f.name == member_name).unwrap();
                self.builder
                    .build_pointer_cast(
                        base_ptr,
                        self.context.ptr_type(AddressSpace::default()),
                        "union_cast",
                    )
                    .unwrap()
            }
            _ => panic!("Expected struct or union type, found {struct_or_union_ty:?}"),
        }
    }

    fn compute_signature_hash(&self, return_type: &Type, param_types: &[Type]) -> u64 {
        let mut sig = format!("{return_type:?}");
        sig.push('(');
        for (i, t) in param_types.iter().enumerate() {
            if i > 0 {
                sig.push(',');
            }
            sig.push_str(&format!("{t:?}"));
        }
        sig.push(')');

        let mut hash = 0xcbf29ce484222325u64;
        for byte in sig.bytes() {
            hash ^= byte as u64;
            hash = hash.wrapping_mul(0x100000001b3u64);
        }
        hash
    }

    fn emit_float_overflow_check(&mut self, val: FloatValue<'ctx>, _span: Span) {
        let func = self.current_fn.unwrap();
        let double_ty = self.context.f64_type();
        let f64_val = if val.get_type() != double_ty {
            self.builder
                .build_float_ext(val, double_ty, "float_ext")
                .unwrap()
        } else {
            val
        };

        // Check if is NaN (compare unordered to itself)
        let is_nan = self
            .builder
            .build_float_compare(inkwell::FloatPredicate::UNO, f64_val, f64_val, "is_nan")
            .unwrap();

        // Check if is Inf (fabs(val) > f64::MAX)
        let max_val = double_ty.const_float(f64::MAX);
        let fabs_fn = self
            .module
            .get_function("llvm.fabs.f64")
            .unwrap_or_else(|| {
                self.module.add_function(
                    "llvm.fabs.f64",
                    double_ty.fn_type(&[double_ty.into()], false),
                    None,
                )
            });
        let abs_val = self
            .builder
            .build_call(fabs_fn, &[f64_val.into()], "abs_val")
            .unwrap()
            .try_as_basic_value()
            .left()
            .unwrap()
            .into_float_value();

        let is_inf = self
            .builder
            .build_float_compare(inkwell::FloatPredicate::OGT, abs_val, max_val, "is_inf")
            .unwrap();
        let is_bad = self.builder.build_or(is_nan, is_inf, "is_bad").unwrap();

        let abort_bb = self
            .context
            .append_basic_block(func, "float_overflow_abort");
        let cont_bb = self.context.append_basic_block(func, "float_overflow_cont");

        self.builder
            .build_conditional_branch(is_bad, abort_bb, cont_bb)
            .unwrap();

        self.builder.position_at_end(abort_bb);
        let msg = self
            .builder
            .build_global_string_ptr("Floating point overflow or NaN", "msg")
            .unwrap();
        let file = self
            .builder
            .build_global_string_ptr(&self.filename, "file")
            .unwrap();
        let line = self.context.i32_type().const_int(0, false);
        self.builder
            .build_call(
                self.abort_fn,
                &[
                    msg.as_pointer_value().into(),
                    file.as_pointer_value().into(),
                    line.into(),
                ],
                "abort",
            )
            .unwrap();
        self.builder.build_unreachable().unwrap();

        self.builder.position_at_end(cont_bb);
    }

    fn is_deref_statically_safe(&self, inner: &Expr) -> bool {
        if let ExprNode::Binary(BinaryOp::Add, left, right) = &inner.node {
            let check_array_index = |arr_expr: &Expr, idx_expr: &Expr| -> bool {
                if let ExprNode::Identifier(name) = &arr_expr.node {
                    if let Some(ty) = self.variable_types.get(name) {
                        let mut current_ty = ty;
                        while let Type::Const(inner_ty) = current_ty {
                            current_ty = inner_ty;
                        }
                        if let Type::Array(_, ArraySize::Const(len)) = current_ty {
                            if let ExprNode::Literal(Literal::Int(idx_val)) = &idx_expr.node {
                                if *idx_val >= 0 && (*idx_val as usize) < *len {
                                    return true;
                                }
                            }
                        }
                    }
                }
                false
            };
            if check_array_index(left, right) || check_array_index(right, left) {
                return true;
            }
        }
        false
    }

    fn compute_type_hash(&self, ty: &Type) -> u64 {
        let sig = format!("{ty:?}");
        let mut hash = 0xcbf29ce484222325u64;
        for byte in sig.bytes() {
            hash ^= byte as u64;
            hash = hash.wrapping_mul(0x100000001b3u64);
        }
        hash
    }

    fn is_std_lib_symbol(&self, name: &str) -> bool {
        let std_syms = [
            "malloc",
            "calloc",
            "realloc",
            "free",
            "aligned_alloc",
            "printf",
            "sprintf",
            "fprintf",
            "scanf",
            "sscanf",
            "fscanf",
            "memcpy",
            "memmove",
            "memset",
            "memcmp",
            "strlen",
            "strcpy",
            "strncpy",
            "strcat",
            "strncat",
            "strcmp",
            "strncmp",
            "exit",
            "abort",
            "signal",
            "alarm",
            "isalpha",
            "isdigit",
            "isalnum",
            "isspace",
            "isupper",
            "islower",
            "stdin",
            "stdout",
            "stderr",
            "main",
        ];
        name.starts_with("__stricc") || name.starts_with("__builtin") || std_syms.contains(&name)
    }

    fn is_string_pointer(&self, ty: &Type) -> bool {
        let mut current = ty;
        while let Type::Const(inner) = current {
            current = inner;
        }
        if let Type::Pointer(inner) = current {
            let mut inner_curr = &**inner;
            while let Type::Const(in_c) = inner_curr {
                inner_curr = in_c;
            }
            matches!(inner_curr, Type::Char | Type::UnsignedChar)
        } else if let Type::Array(inner, _) = current {
            let mut inner_curr = &**inner;
            while let Type::Const(in_c) = inner_curr {
                inner_curr = in_c;
            }
            matches!(inner_curr, Type::Char | Type::UnsignedChar)
        } else {
            false
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
}
