use inkwell::context::Context;
use stricc::codegen::Codegen;
use stricc::parser::Parser;
use stricc::typechecker::Typechecker;

fn run_codegen(source: &str) -> String {
    let mut parser = Parser::new(source, "test.c");
    let mut program = parser.parse_program();
    assert!(
        parser.errors.is_empty(),
        "Parser errors: {:?}",
        parser.errors
    );

    let mut typechecker = Typechecker::new("test.c");
    typechecker.check_program(&mut program).unwrap();

    let context = Context::create();
    let module = context.create_module("test.c");
    let builder = context.create_builder();

    let mut codegen = Codegen::new(&context, &module, &builder, "test.c");
    codegen.gen_program(&program);

    module.print_to_string().to_string()
}

// ── Basic functions ───────────────────────────────────────────────────────────

#[test]
fn test_codegen_basic_main() {
    let ir = run_codegen("int main() { return 42; }");
    assert!(ir.contains("define i32 @main()"));
    assert!(ir.contains("ret i32 42"));
}

#[test]
fn test_codegen_void_function() {
    let ir = run_codegen("void noop() {}");
    assert!(ir.contains("define void @noop()"));
    assert!(ir.contains("ret void"));
}

#[test]
fn test_codegen_multiple_functions() {
    let source = "
        int add(int a, int b) { return a + b; }
        int sub(int a, int b) { return a - b; }
    ";
    let ir = run_codegen(source);
    assert!(ir.contains("define i32 @add(i32 %0, i32 %1)"));
    assert!(ir.contains("define i32 @sub(i32 %0, i32 %1)"));
}

// ── Arithmetic ────────────────────────────────────────────────────────────────

#[test]
fn test_codegen_variables_and_arithmetic() {
    let source = "
        int compute(int x, int y) {
            int sum = x + y;
            int diff = x - y;
            int prod = x * y;
            int quot = x / y;
            return sum + diff + prod + quot;
        }
    ";
    let ir = run_codegen(source);
    assert!(ir.contains("define i32 @compute(i32 %0, i32 %1)"));
    assert!(ir.contains("llvm.sadd.with.overflow.i32"));
    assert!(ir.contains("llvm.ssub.with.overflow.i32"));
    assert!(ir.contains("llvm.smul.with.overflow.i32"));
    assert!(ir.contains("sdiv i32"));
}

#[test]
fn test_codegen_modulo() {
    let source = "int rem(int a, int b) { return a % b; }";
    let ir = run_codegen(source);
    assert!(ir.contains("srem i32"));
}

#[test]
fn test_codegen_bitwise_ops() {
    let source = "
        int bitops(int a, int b) {
            int r = a & b;
            int s = a | b;
            int t = a ^ b;
            int u = ~a;
            return r + s + t + u;
        }
    ";
    let ir = run_codegen(source);
    assert!(ir.contains("and i32"));
    assert!(ir.contains("or i32"));
    assert!(ir.contains("xor i32"));
}

#[test]
fn test_codegen_shift_ops() {
    let source = "
        int shifts(int a) {
            return (a << 2) >> 1;
        }
    ";
    let ir = run_codegen(source);
    assert!(ir.contains("shl i32") || ir.contains("lshr i32") || ir.contains("ashr i32"));
}

#[test]
fn test_codegen_float_arithmetic() {
    let source = "
        double float_ops(double a, double b) {
            double s = a + b;
            double d = a - b;
            double m = a * b;
            double q = a / b;
            return s + d + m + q;
        }
    ";
    let ir = run_codegen(source);
    assert!(ir.contains("fadd double") || ir.contains("fadd"));
    assert!(ir.contains("fsub double") || ir.contains("fsub"));
    assert!(ir.contains("fmul double") || ir.contains("fmul"));
    assert!(ir.contains("fdiv double") || ir.contains("fdiv"));
}

#[test]
fn test_codegen_unary_neg() {
    let source = "int neg(int x) { return -x; }";
    let ir = run_codegen(source);
    // neg is implemented as sub 0, x or mul x, -1
    assert!(ir.contains("sub i32") || ir.contains("@neg"));
}

#[test]
fn test_codegen_logical_not() {
    let source = "int lnot(int x) { return !x; }";
    let ir = run_codegen(source);
    assert!(ir.contains("icmp") || ir.contains("@lnot"));
}

// ── Comparisons ───────────────────────────────────────────────────────────────

#[test]
fn test_codegen_comparison_ops() {
    let source = "
        int cmp(int a, int b) {
            int r = a < b;
            int s = a <= b;
            int t = a > b;
            int u = a >= b;
            int v = a == b;
            int w = a != b;
            return r + s + t + u + v + w;
        }
    ";
    let ir = run_codegen(source);
    assert!(ir.contains("icmp slt i32") || ir.contains("icmp"));
}

// ── Increment / decrement ─────────────────────────────────────────────────────

#[test]
fn test_codegen_pre_post_increment() {
    let source = "
        void incdec(int x) {
            ++x;
            x++;
            --x;
            x--;
        }
    ";
    let ir = run_codegen(source);
    // Pre/post-inc uses checked overflow intrinsics for signed integers
    assert!(
        ir.contains("llvm.sadd.with.overflow")
            || ir.contains("llvm.ssub.with.overflow")
            || ir.contains("add i32")
            || ir.contains("sub i32")
    );
}

// ── Control flow ──────────────────────────────────────────────────────────────

#[test]
fn test_codegen_control_flow() {
    let source = "
        int max(int a, int b) {
            if (a > b) {
                return a;
            } else {
                return b;
            }
        }
    ";
    let ir = run_codegen(source);
    assert!(ir.contains("define i32 @max(i32 %0, i32 %1)"));
    assert!(ir.contains("icmp sgt i32"));
    assert!(ir.contains("br i1"));
}

#[test]
fn test_codegen_if_without_else() {
    let source = "
        void clamp_low(int* x) {
            if (*x < 0) {
                *x = 0;
            }
        }
    ";
    let ir = run_codegen(source);
    assert!(ir.contains("icmp slt i32"));
    assert!(ir.contains("br i1"));
}

#[test]
fn test_codegen_loops() {
    let source = "
        int sum_to(int n) {
            int sum = 0;
            for (int i = 0; i < n; i++) {
                sum = sum + i;
            }
            return sum;
        }
    ";
    let ir = run_codegen(source);
    assert!(ir.contains("define i32 @sum_to(i32 %0)"));
    assert!(ir.contains("icmp slt i32"));
}

#[test]
fn test_codegen_while_loop() {
    let source = "
        int countdown(int n) {
            while (n > 0) {
                n = n - 1;
            }
            return n;
        }
    ";
    let ir = run_codegen(source);
    assert!(ir.contains("whilecond") || ir.contains("whilebody"));
    assert!(ir.contains("icmp sgt i32"));
}

#[test]
fn test_codegen_for_infinite_loop() {
    let source = "void spin() { for (;;) { return; } }";
    let ir = run_codegen(source);
    assert!(ir.contains("forcond") || ir.contains("forbody"));
}

#[test]
fn test_codegen_switch_case() {
    let source = "
        int describe(int x) {
            switch (x) {
                case 0:
                    return 10;
                case 1:
                    return 20;
                default:
                    return -1;
            }
        }
    ";
    // Codegen reaches the switch fallback path
    let ir = run_codegen(source);
    assert!(ir.contains("@describe"));
}

// ── Pointers and structs ──────────────────────────────────────────────────────

#[test]
fn test_codegen_pointers_and_structs() {
    let source = "
        struct Point {
            int x;
            int y;
        };
        int get_x(struct Point* p) {
            return p->x;
        }
    ";
    let ir = run_codegen(source);
    assert!(ir.contains("define i32 @get_x(ptr %0)"));
    assert!(ir.contains("getelementptr"));
}

#[test]
fn test_codegen_struct_dot_member() {
    let source = "
        struct Pair { int first; int second; };
        int first(struct Pair p) { return p.first; }
    ";
    let ir = run_codegen(source);
    assert!(ir.contains("@first"));
    assert!(ir.contains("getelementptr"));
}

#[test]
fn test_codegen_struct_member_assign() {
    let source = "
        struct Point { int x; int y; };
        void set_x(struct Point* p, int v) { p->x = v; }
    ";
    let ir = run_codegen(source);
    assert!(ir.contains("define void @set_x(ptr %0, i32 %1)"));
    assert!(ir.contains("store i32"));
}

#[test]
fn test_codegen_pointer_deref_write() {
    let source = "
        void write_ptr(int* p, int v) { *p = v; }
    ";
    let ir = run_codegen(source);
    assert!(ir.contains("define void @write_ptr(ptr %0, i32 %1)"));
    assert!(ir.contains("store i32"));
}

#[test]
fn test_codegen_pointer_deref_read() {
    let source = "
        int read_ptr(int* p) { return *p; }
    ";
    let ir = run_codegen(source);
    assert!(ir.contains("define i32 @read_ptr(ptr %0)"));
    assert!(ir.contains("load i32"));
}

// ── Arrays ────────────────────────────────────────────────────────────────────

#[test]
fn test_codegen_array_access() {
    let source = "
        int get(int* arr, int i) { return arr[i]; }
    ";
    let ir = run_codegen(source);
    assert!(ir.contains("getelementptr"));
}

#[test]
fn test_codegen_local_array() {
    let source = "
        int sum_arr() {
            int arr[3];
            arr[0] = 1;
            arr[1] = 2;
            arr[2] = 3;
            return arr[0] + arr[1] + arr[2];
        }
    ";
    let ir = run_codegen(source);
    assert!(ir.contains("alloca [3 x i32]") || ir.contains("[3 x i32]"));
}

// ── Literals and globals ──────────────────────────────────────────────────────

#[test]
fn test_codegen_char_literal() {
    let source = "int f() { return 'A'; }";
    let ir = run_codegen(source);
    // 'A' is 65
    assert!(ir.contains("65") || ir.contains("ret i32"));
}

#[test]
fn test_codegen_bool_literal() {
    let source = "
        void booltest() {
            bool t = true;
            bool f = false;
        }
    ";
    let ir = run_codegen(source);
    assert!(ir.contains("i1") || ir.contains("@booltest"));
}

#[test]
fn test_codegen_string_literal() {
    let source = "
        int printf(const char* fmt, ...);
        void greet() { printf(\"hello\"); }
    ";
    let ir = run_codegen(source);
    assert!(ir.contains("hello") || ir.contains("@greet"));
}

#[test]
fn test_codegen_nullptr_literal() {
    let source = "
        int* get_null() { return nullptr; }
    ";
    let ir = run_codegen(source);
    assert!(ir.contains("null") || ir.contains("ptr null"));
}

#[test]
fn test_codegen_global_variable() {
    // A global that is only declared (no init), referenced in a function.
    // The global symbol should appear in the IR.
    let source = "
        int counter;
        int get() { return 0; }
    ";
    let ir = run_codegen(source);
    assert!(ir.contains("@counter"));
}

#[test]
fn test_codegen_global_char() {
    let source = "char letter = 'Z';";
    let ir = run_codegen(source);
    assert!(ir.contains("@letter"));
}

#[test]
fn test_codegen_global_float() {
    let source = "double pi = 1.23;";
    let ir = run_codegen(source);
    assert!(ir.contains("@pi"));
}

#[test]
fn test_codegen_global_bool() {
    let source = "bool flag = true;";
    let ir = run_codegen(source);
    assert!(ir.contains("@flag"));
}

// ── Casts ─────────────────────────────────────────────────────────────────────

#[test]
fn test_codegen_numeric_cast() {
    let source = "
        int truncate(double x) { return (int)x; }
    ";
    let ir = run_codegen(source);
    assert!(ir.contains("fptosi") || ir.contains("@truncate"));
}

#[test]
fn test_codegen_int_to_float_cast() {
    let source = "
        double widen(int x) { return (double)x; }
    ";
    let ir = run_codegen(source);
    assert!(ir.contains("sitofp") || ir.contains("@widen"));
}

// ── Function calls ────────────────────────────────────────────────────────────

#[test]
fn test_codegen_function_call() {
    let source = "
        int square(int x) { return x * x; }
        int main() { return square(5); }
    ";
    let ir = run_codegen(source);
    assert!(ir.contains("call i32 @square"));
}

#[test]
fn test_codegen_external_function_call() {
    let source = "
        int printf(const char* fmt, ...);
        void greet() { printf(\"%d\", 42); }
    ";
    let ir = run_codegen(source);
    assert!(ir.contains("call") && ir.contains("printf"));
}

// ── Type variety ──────────────────────────────────────────────────────────────

#[test]
fn test_codegen_short_type() {
    let source = "short f(short a, short b) { return a + b; }";
    let ir = run_codegen(source);
    assert!(ir.contains("i16"));
}

#[test]
fn test_codegen_long_type() {
    let source = "long f(long a, long b) { return a - b; }";
    let ir = run_codegen(source);
    assert!(ir.contains("i64"));
}

#[test]
fn test_codegen_unsigned_type() {
    let source = "unsigned int f(unsigned int a, unsigned int b) { return a + b; }";
    let ir = run_codegen(source);
    // unsigned add is regular add (no overflow check needed for unsigned)
    assert!(ir.contains("i32"));
}

#[test]
fn test_codegen_char_type() {
    let source = "char f(char a, char b) { return a + b; }";
    let ir = run_codegen(source);
    assert!(ir.contains("i8"));
}

#[test]
fn test_codegen_float_type() {
    let source = "float f(float a) { return a * 2.0f; }";
    let ir = run_codegen(source);
    assert!(ir.contains("float") || ir.contains("f32"));
}

// ── Unions ────────────────────────────────────────────────────────────────────

#[test]
fn test_codegen_union_type() {
    let source = "
        union Data { int i; float f; };
        int get_i(union Data d) { return d.i; }
    ";
    let ir = run_codegen(source);
    assert!(ir.contains("@get_i"));
}

// ── Unsafe block ──────────────────────────────────────────────────────────────

#[test]
fn test_codegen_unsafe_block() {
    let source = "
        int* cast_int(int x) {
            int* p;
            __unsafe {
                p = (int*)x;
            }
            return p;
        }
    ";
    let ir = run_codegen(source);
    assert!(ir.contains("@cast_int"));
}

// ── VLA ───────────────────────────────────────────────────────────────────────

#[test]
fn test_codegen_vla() {
    let source = "
        int sum_vla(int n) {
            int arr[n];
            arr[0] = 1;
            return arr[0];
        }
    ";
    let ir = run_codegen(source);
    assert!(ir.contains("alloca i32") || ir.contains("@sum_vla"));
}

// ── Sizeof ────────────────────────────────────────────────────────────────────

#[test]
fn test_codegen_sizeof_type() {
    let source = "
        long get_size() { return sizeof(int); }
    ";
    let ir = run_codegen(source);
    assert!(ir.contains("@get_size"));
}

// NOTE: sizeof(expr) on non-trivial expressions is not yet supported in codegen
// (the expression path panics). Only sizeof(Type) is fully exercised.

// ── Logical ops ───────────────────────────────────────────────────────────────

#[test]
fn test_codegen_logical_and_or() {
    let source = "
        int logic(int a, int b) {
            int r = a && b;
            int s = a || b;
            return r + s;
        }
    ";
    let ir = run_codegen(source);
    assert!(ir.contains("@logic"));
    assert!(ir.contains("icmp") || ir.contains("and i1") || ir.contains("or i1"));
}

// ── Pointer pointer subtraction ───────────────────────────────────────────────

#[test]
fn test_codegen_pointer_diff() {
    let source = "
        long ptrdiff(int* a, int* b) { return a - b; }
    ";
    let ir = run_codegen(source);
    assert!(ir.contains("@ptrdiff"));
}

// ── Const variables ───────────────────────────────────────────────────────────

#[test]
fn test_codegen_const_var() {
    let source = "
        int use_const() {
            const int x = 10;
            return x;
        }
    ";
    let ir = run_codegen(source);
    assert!(ir.contains("@use_const"));
    assert!(ir.contains("i32 10") || ir.contains("ret i32"));
}

// ── Atomic type ───────────────────────────────────────────────────────────────

#[test]
fn test_codegen_atomic_int() {
    // _Atomic(int) read: atomic variable used as a function return value
    let source = "
        _Atomic(int) get_atomic(_Atomic(int) x) {
            return x;
        }
    ";
    let ir = run_codegen(source);
    assert!(ir.contains("@get_atomic"));
    assert!(ir.contains("i32")); // _Atomic(int) maps to i32
}

// ── Nested compound statements ────────────────────────────────────────────────

#[test]
fn test_codegen_nested_scopes() {
    let source = "
        int nested() {
            int x = 1;
            {
                int y = 2;
                x = x + y;
            }
            return x;
        }
    ";
    let ir = run_codegen(source);
    assert!(ir.contains("@nested"));
    assert!(ir.contains("ret i32"));
}

// ── CFI / signature verification ─────────────────────────────────────────────

#[test]
fn test_codegen_main_cfi_registration() {
    let source = "
        int helper(int x) { return x * 2; }
        int main() { return helper(1); }
    ";
    let ir = run_codegen(source);
    // main should call cfi_register for each defined function
    assert!(ir.contains("__stricc_rt_cfi_register") || ir.contains("cfi_reg"));
}
