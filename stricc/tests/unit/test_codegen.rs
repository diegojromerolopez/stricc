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

#[test]
fn test_codegen_basic_main() {
    let ir = run_codegen("int main() { return 42; }");
    assert!(ir.contains("define i32 @main()"));
    assert!(ir.contains("ret i32 42"));
}

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
