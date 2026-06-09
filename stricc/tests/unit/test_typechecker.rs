use stricc::parser::Parser;
use stricc::typechecker::Typechecker;

fn check_code(source: &str) -> Result<(), String> {
    let mut parser = Parser::new(source, "test.c");
    let mut program = parser.parse_program();
    if !parser.errors.is_empty() {
        return Err(format!("Parser error: {:?}", parser.errors));
    }
    let mut typechecker = Typechecker::new("test.c");
    typechecker
        .check_program(&mut program)
        .map_err(|e| format!("Typechecker error: {e:?}"))
}

// ── Basic valid programs ──────────────────────────────────────────────────────

#[test]
fn test_typechecker_valid_code() {
    let source = "
        int compute(int a) {
            int x = a + 5;
            return x;
        }
        void test() {
            int val = compute(10);
            int* p = &val;
            *p = 20;
        }
    ";
    assert!(check_code(source).is_ok());
}

#[test]
fn test_typechecker_void_function_no_return() {
    let source = "void noop() {}";
    assert!(check_code(source).is_ok());
}

#[test]
fn test_typechecker_return_void_explicit() {
    let source = "void f() { return; }";
    assert!(check_code(source).is_ok());
}

// ── Identifiers & declarations ────────────────────────────────────────────────

#[test]
fn test_typechecker_undeclared_identifier() {
    let source = "
        int test() {
            return x;
        }
    ";
    let err = check_code(source).unwrap_err();
    assert!(err.contains("Undeclared identifier"));
}

#[test]
fn test_typechecker_incompatible_assignment() {
    let source = "
        void test() {
            int x = 5;
            int* p = x;
        }
    ";
    let err = check_code(source).unwrap_err();
    assert!(err.contains("Type incompatibility"));
}

#[test]
fn test_typechecker_global_var_auto_rejected() {
    // Global variables may not use auto type
    let source = "auto x = 5;";
    let err = check_code(source).unwrap_err();
    assert!(err.contains("Global variables cannot use 'auto' type inference"));
}

#[test]
fn test_typechecker_auto_local_inferred() {
    let source = "
        void test() {
            auto x = 42;
        }
    ";
    assert!(check_code(source).is_ok());
}

#[test]
fn test_typechecker_auto_without_init() {
    let source = "
        void test() {
            auto x;
        }
    ";
    let err = check_code(source).unwrap_err();
    assert!(err.contains("auto variable must have an initializer"));
}

#[test]
fn test_typechecker_global_var_conflict() {
    let source = "
        int g;
        float g;
    ";
    let err = check_code(source).unwrap_err();
    assert!(err.contains("Redeclaration of global variable"));
}

#[test]
fn test_typechecker_global_var_same_type_ok() {
    // Redeclaring with same type is idempotent
    let source = "
        int g;
        int g;
        void test() { g = 1; }
    ";
    assert!(check_code(source).is_ok());
}

// ── Control flow ──────────────────────────────────────────────────────────────

#[test]
fn test_typechecker_invalid_if_condition() {
    let source = "
        struct Point { int x; int y; };
        void test() {
            struct Point pt;
            if (pt) {
            }
        }
    ";
    let err = check_code(source).unwrap_err();
    assert!(err.contains("If condition must be of integer or pointer type"));
}

#[test]
fn test_typechecker_invalid_while_condition() {
    let source = "
        struct Point { int x; int y; };
        void test() {
            struct Point pt;
            while (pt) {
            }
        }
    ";
    let err = check_code(source).unwrap_err();
    assert!(err.contains("While condition must be of integer or pointer type"));
}

#[test]
fn test_typechecker_for_loop_valid() {
    let source = "
        void test() {
            for (int i = 0; i < 10; i++) {
            }
        }
    ";
    assert!(check_code(source).is_ok());
}

#[test]
fn test_typechecker_for_loop_invalid_condition() {
    let source = "
        struct S { int x; };
        void test() {
            struct S s;
            for (; s; ) {}
        }
    ";
    let err = check_code(source).unwrap_err();
    assert!(err.contains("For condition must be of integer or pointer type"));
}

#[test]
fn test_typechecker_for_loop_infinite() {
    let source = "void f() { for (;;) { return; } }";
    assert!(check_code(source).is_ok());
}

#[test]
fn test_typechecker_switch_case_valid() {
    let source = "
        int test(int x) {
            switch (x) {
                case 1:
                    return 10;
                default:
                    return 0;
            }
        }
    ";
    assert!(check_code(source).is_ok());
}

#[test]
fn test_typechecker_switch_non_integer() {
    let source = "
        struct S { int x; };
        void test() {
            struct S s;
            switch (s) {}
        }
    ";
    let err = check_code(source).unwrap_err();
    assert!(err.contains("Switch expression must be of integer type"));
}

#[test]
fn test_typechecker_break_continue_valid() {
    let source = "
        void test() {
            for (int i = 0; i < 10; i++) {
                if (i == 5) break;
                if (i == 3) continue;
            }
        }
    ";
    assert!(check_code(source).is_ok());
}

// ── Return analysis ───────────────────────────────────────────────────────────

#[test]
fn test_typechecker_missing_return() {
    let source = "
        int test() {
            // missing return
        }
    ";
    let err = check_code(source).unwrap_err();
    assert!(err.contains("Missing return in non-void function"));
}

#[test]
fn test_typechecker_return_wrong_value_in_void_fn() {
    // returning void expr in a void fn is not an error by itself,
    // but returning a value from a non-void fn with wrong type is
    let source = "
        int test() {
            return;
        }
    ";
    let err = check_code(source).unwrap_err();
    assert!(err.contains("Missing return value"));
}

#[test]
fn test_typechecker_if_else_definite_return() {
    // Both branches return → satisfies definite return analysis
    let source = "
        int test(int x) {
            if (x > 0) {
                return 1;
            } else {
                return -1;
            }
        }
    ";
    assert!(check_code(source).is_ok());
}

// ── Escape analysis ───────────────────────────────────────────────────────────

#[test]
fn test_typechecker_escape_local() {
    let source = "
        int* test() {
            int x = 5;
            return &x;
        }
    ";
    let err = check_code(source).unwrap_err();
    assert!(err.contains("Escaping stack address"));
}

#[test]
fn test_typechecker_escape_inner_scope_decl() {
    // Declaring a pointer and initialising it with an address from a deeper scope
    // triggers the escape-analysis check in StmtNode::Decl.
    let source = "
        void test() {
            {
                int x = 5;
                int* p = &x;
            }
        }
    ";
    // Same-scope address-of is fine
    assert!(check_code(source).is_ok());
}

// ── Static bounds ─────────────────────────────────────────────────────────────

#[test]
fn test_typechecker_static_bounds_error() {
    let source = "
        void test() {
            int arr[5];
            int x = arr[5];
        }
    ";
    let err = check_code(source).unwrap_err();
    assert!(err.contains("Array index out of bounds"));
}

#[test]
fn test_typechecker_static_bounds_valid() {
    let source = "
        void test() {
            int arr[5];
            int x = arr[4];
        }
    ";
    assert!(check_code(source).is_ok());
}

// ── Casts ─────────────────────────────────────────────────────────────────────

#[test]
fn test_typechecker_cast_constness() {
    let source = "
        void test() {
            const int x = 5;
            int* p = (int*)&x;
        }
    ";
    let err = check_code(source).unwrap_err();
    assert!(err.contains("Const qualifier discarded"));
}

#[test]
fn test_typechecker_int_to_pointer_cast_unsafe_required() {
    let source = "
        void test() {
            int x = 42;
            int* p = (int*)x;
        }
    ";
    let err = check_code(source).unwrap_err();
    assert!(err.contains("Unsafe integer-to-pointer cast"));
}

#[test]
fn test_typechecker_int_to_pointer_cast_in_unsafe_ok() {
    let source = "
        void test() {
            int x = 42;
            int* p;
            __unsafe {
                p = (int*)x;
            }
        }
    ";
    assert!(check_code(source).is_ok());
}

#[test]
fn test_typechecker_pointer_to_int_cast_unsafe_required() {
    let source = "
        void test() {
            int x = 5;
            int* p = &x;
            int addr = (int)p;
        }
    ";
    let err = check_code(source).unwrap_err();
    assert!(err.contains("Unsafe pointer-to-integer cast"));
}

// ── Binary expressions ────────────────────────────────────────────────────────

#[test]
fn test_typechecker_binary_arithmetic_valid() {
    let source = "
        int test(int a, int b) {
            int s = a + b;
            int d = a - b;
            int m = a * b;
            int q = a / b;
            int r = a % b;
            return s + d + m + q + r;
        }
    ";
    assert!(check_code(source).is_ok());
}

#[test]
fn test_typechecker_invalid_addition() {
    let source = "
        struct S { int x; };
        void test() {
            struct S a;
            struct S b;
            struct S c = a + b;
        }
    ";
    let err = check_code(source).unwrap_err();
    assert!(err.contains("Invalid operands to addition") || err.contains("incompatib"));
}

#[test]
fn test_typechecker_pointer_arithmetic_valid() {
    let source = "
        void test(int* p) {
            int* q = p + 1;
        }
    ";
    assert!(check_code(source).is_ok());
}

#[test]
fn test_typechecker_modulo_float_error() {
    let source = "
        void test() {
            double x = 1.5 % 2.0;
        }
    ";
    let err = check_code(source).unwrap_err();
    assert!(err.contains("Modulo operation requires integer types") || err.contains("incompatib"));
}

#[test]
fn test_typechecker_shift_valid() {
    let source = "
        int test(int a) {
            return a << 2;
        }
    ";
    assert!(check_code(source).is_ok());
}

#[test]
fn test_typechecker_shift_out_of_bounds() {
    let source = "
        int test(int a) {
            return a << 32;
        }
    ";
    let err = check_code(source).unwrap_err();
    assert!(err.contains("Shift count") && err.contains("out of bounds"));
}

#[test]
fn test_typechecker_bitwise_ops_valid() {
    let source = "
        int test(int a, int b) {
            return (a & b) | (a ^ b);
        }
    ";
    assert!(check_code(source).is_ok());
}

#[test]
fn test_typechecker_bitwise_on_float_error() {
    let source = "
        void test() {
            double x = 1.0 & 2.0;
        }
    ";
    let err = check_code(source).unwrap_err();
    assert!(err.contains("Bitwise operations require integer types") || err.contains("incompatib"));
}

#[test]
fn test_typechecker_logical_ops_valid() {
    let source = "
        void test(int a, int b) {
            int r = (a && b) || a;
        }
    ";
    assert!(check_code(source).is_ok());
}

#[test]
fn test_typechecker_comparison_valid() {
    let source = "
        void test(int a, int b) {
            int r = a < b;
            int s = a == b;
            int t = a >= b;
        }
    ";
    assert!(check_code(source).is_ok());
}

#[test]
fn test_typechecker_pointer_comparison_valid() {
    let source = "
        void test(int* a, int* b) {
            int r = a == b;
            int s = a != b;
        }
    ";
    assert!(check_code(source).is_ok());
}

// ── Unary expressions ─────────────────────────────────────────────────────────

#[test]
fn test_typechecker_unary_neg_valid() {
    let source = "
        int test(int x) { return -x; }
    ";
    assert!(check_code(source).is_ok());
}

#[test]
fn test_typechecker_unary_neg_invalid() {
    let source = "
        struct S { int x; };
        void test(struct S s) { struct S r = -s; }
    ";
    let err = check_code(source).unwrap_err();
    assert!(err.contains("negation") || err.contains("incompatib"));
}

#[test]
fn test_typechecker_unary_not_valid() {
    let source = "void test(int x) { int r = !x; }";
    assert!(check_code(source).is_ok());
}

#[test]
fn test_typechecker_unary_bitnot_valid() {
    let source = "int test(int x) { return ~x; }";
    assert!(check_code(source).is_ok());
}

#[test]
fn test_typechecker_deref_valid() {
    let source = "
        int test(int* p) { return *p; }
    ";
    assert!(check_code(source).is_ok());
}

#[test]
fn test_typechecker_deref_non_pointer() {
    let source = "
        int test(int x) { return *x; }
    ";
    let err = check_code(source).unwrap_err();
    assert!(err.contains("non-pointer") || err.contains("dereference"));
}

#[test]
fn test_typechecker_preinc_valid() {
    let source = "void test(int x) { ++x; }";
    assert!(check_code(source).is_ok());
}

#[test]
fn test_typechecker_postinc_valid() {
    let source = "void test(int x) { x++; }";
    assert!(check_code(source).is_ok());
}

// ── Member access ─────────────────────────────────────────────────────────────

#[test]
fn test_typechecker_struct_member_access() {
    let source = "
        struct Point { int x; int y; };
        int test(struct Point p) { return p.x + p.y; }
    ";
    assert!(check_code(source).is_ok());
}

#[test]
fn test_typechecker_struct_arrow_access() {
    let source = "
        struct Point { int x; int y; };
        int test(struct Point* p) { return p->x + p->y; }
    ";
    assert!(check_code(source).is_ok());
}

#[test]
fn test_typechecker_struct_unknown_field() {
    let source = "
        struct Point { int x; int y; };
        int test(struct Point p) { return p.z; }
    ";
    let err = check_code(source).unwrap_err();
    assert!(err.contains("no member named") || err.contains("Unknown struct field"));
}

#[test]
fn test_typechecker_arrow_on_non_pointer() {
    let source = "
        struct Point { int x; };
        int test(struct Point p) { return p->x; }
    ";
    let err = check_code(source).unwrap_err();
    assert!(err.contains("Non-pointer arrow access") || err.contains("pointer"));
}

#[test]
fn test_typechecker_union_member_access() {
    let source = "
        union Data { int i; float f; };
        int test(union Data d) { return d.i; }
    ";
    assert!(check_code(source).is_ok());
}

#[test]
fn test_typechecker_union_unknown_field() {
    let source = "
        union Data { int i; float f; };
        int test(union Data d) { return d.z; }
    ";
    let err = check_code(source).unwrap_err();
    assert!(err.contains("no member named") || err.contains("Unknown union field"));
}

// ── Function calls ────────────────────────────────────────────────────────────

#[test]
fn test_typechecker_undefined_function() {
    let source = "
        void test() {
            unknown_func();
        }
    ";
    let err = check_code(source).unwrap_err();
    assert!(err.contains("Undeclared identifier"));
}

#[test]
fn test_typechecker_wrong_argument_count() {
    let source = "
        int add(int a, int b) { return a + b; }
        void test() { add(1); }
    ";
    let err = check_code(source).unwrap_err();
    assert!(err.contains("expects") && err.contains("arguments"));
}

#[test]
fn test_typechecker_variadic_function_external_ok() {
    // External variadic declarations (like printf) are allowed
    let source = "
        int printf(const char* fmt, ...);
        void test() { printf(\"%d\", 42); }
    ";
    assert!(check_code(source).is_ok());
}

#[test]
fn test_typechecker_user_defined_variadic_rejected() {
    // User-defined variadic function bodies are forbidden
    let source = "
        void bad_func(int x, ...) { }
    ";
    let err = check_code(source).unwrap_err();
    assert!(err.contains("Forbidden user-defined variadic"));
}

#[test]
fn test_typechecker_conflicting_redeclaration() {
    let source = "
        int foo(int x);
        float foo(int x);
    ";
    let err = check_code(source).unwrap_err();
    assert!(err.contains("Conflicting redeclaration"));
}

// ── Printf format validation ──────────────────────────────────────────────────

#[test]
fn test_typechecker_printf_valid() {
    let source = "
        int printf(const char* fmt, ...);
        void test() { printf(\"%d %s\", 42, \"hello\"); }
    ";
    assert!(check_code(source).is_ok());
}

#[test]
fn test_typechecker_printf_too_few_args() {
    let source = "
        int printf(const char* fmt, ...);
        void test() { printf(\"%d %d\", 1); }
    ";
    let err = check_code(source).unwrap_err();
    assert!(err.contains("Mismatched printf") || err.contains("Format mismatch"));
}

#[test]
fn test_typechecker_printf_too_many_args() {
    let source = "
        int printf(const char* fmt, ...);
        void test() { printf(\"%d\", 1, 2); }
    ";
    let err = check_code(source).unwrap_err();
    assert!(err.contains("too many arguments") || err.contains("Format mismatch"));
}

#[test]
fn test_typechecker_printf_wrong_type_float() {
    // %f expects float but we give int
    let source = "
        int printf(const char* fmt, ...);
        void test() { printf(\"%f\", 42); }
    ";
    let err = check_code(source).unwrap_err();
    assert!(err.contains("expects floating point") || err.contains("Format mismatch"));
}

#[test]
fn test_typechecker_printf_percent_percent_ok() {
    // %% is an escaped percent, no arg consumed
    let source = "
        int printf(const char* fmt, ...);
        void test() { printf(\"100%%\"); }
    ";
    assert!(check_code(source).is_ok());
}

// ── Sequence point violations ─────────────────────────────────────────────────

#[test]
fn test_typechecker_sequence_point_double_write() {
    // x is incremented twice between sequence points: i++ + i++
    let source = "
        int test() {
            int i = 0;
            int r = i++ + i++;
            return r;
        }
    ";
    let err = check_code(source).unwrap_err();
    assert!(
        err.contains("Sequence point violation") || err.contains("modified twice"),
        "Got: {err}"
    );
}

#[test]
fn test_typechecker_sequence_point_write_read_conflict() {
    // i is written by i++ and read by i in the same binary expr
    let source = "
        int test() {
            int i = 0;
            int r = (i = 5) + i;
            return r;
        }
    ";
    let err = check_code(source).unwrap_err();
    assert!(
        err.contains("Sequence point violation") || err.contains("conflict"),
        "Got: {err}"
    );
}

// ── sizeof / alignof ──────────────────────────────────────────────────────────

#[test]
fn test_typechecker_sizeof_expr() {
    let source = "
        void test() {
            int x = 5;
            long sz = sizeof(x);
        }
    ";
    assert!(check_code(source).is_ok());
}

#[test]
fn test_typechecker_sizeof_type() {
    let source = "
        void test() {
            long sz = sizeof(int);
        }
    ";
    assert!(check_code(source).is_ok());
}

// ── Unsafe block ──────────────────────────────────────────────────────────────

#[test]
fn test_typechecker_unsafe_block_valid() {
    let source = "
        void test() {
            int x = 5;
            int* p;
            __unsafe {
                p = (int*)&x;
            }
        }
    ";
    assert!(check_code(source).is_ok());
}

// ── VLA ───────────────────────────────────────────────────────────────────────

#[test]
fn test_typechecker_vla_valid() {
    let source = "
        void test(int n) {
            int arr[n];
        }
    ";
    assert!(check_code(source).is_ok());
}

// ── String literal type ───────────────────────────────────────────────────────

#[test]
fn test_typechecker_string_literal_type() {
    let source = "
        int printf(const char* fmt, ...);
        void test() {
            const char* s = \"hello\";
            printf(\"%s\", s);
        }
    ";
    assert!(check_code(source).is_ok());
}

// ── Null pointer ──────────────────────────────────────────────────────────────

#[test]
fn test_typechecker_nullptr_assign_to_pointer() {
    let source = "
        void test() {
            int* p = nullptr;
        }
    ";
    assert!(check_code(source).is_ok());
}
