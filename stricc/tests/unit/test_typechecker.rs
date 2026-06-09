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
