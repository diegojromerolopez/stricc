use stricc::parser::Parser;

#[test]
fn test_parser_basic_program() {
    let source = "int main() { return 0; }";
    let mut parser = Parser::new(source, "test.c");
    let program = parser.parse_program();
    assert!(parser.errors.is_empty());
    assert_eq!(program.decls.len(), 1);
}

#[test]
fn test_parser_struct_and_union() {
    let source = "
        struct Point {
            int x;
            int y;
        };
        union Data {
            int i;
            float f;
        };
    ";
    let mut parser = Parser::new(source, "test.c");
    let program = parser.parse_program();
    assert!(parser.errors.is_empty());
    assert_eq!(program.decls.len(), 2);
}

#[test]
fn test_parser_global_variable() {
    let source = "int g_x = 42; const char* g_s = \"hello\";";
    let mut parser = Parser::new(source, "test.c");
    let program = parser.parse_program();
    assert!(parser.errors.is_empty());
    assert_eq!(program.decls.len(), 2);
}

#[test]
fn test_parser_statements() {
    let source = "
        void test() {
            if (1) {
                int x = 5;
            } else {
                int y = 10;
            }
            while (0) {
                break;
            }
            for (int i = 0; i < 10; i++) {
                continue;
            }
            switch (1) {
                case 1:
                    return;
                default:
                    break;
            }
            __unsafe {
                int* p = nullptr;
            }
        }
    ";
    let mut parser = Parser::new(source, "test.c");
    let _program = parser.parse_program();
    assert!(parser.errors.is_empty());
}

#[test]
fn test_parser_expressions() {
    let source = "
        int compute() {
            int val = (5 + 3) * 2 / 1 % 3;
            val = val & 1 | 2 ^ 3;
            val = val && 0 || 1;
            val = val == 1 != 0;
            val = val < 5 <= 10 > 0 >= 2;
            int* p = &val;
            int deref = *p;
            struct Point pt;
            int mx = pt.x;
            struct Point* ppt = &pt;
            int my = ppt->y;
            int sz = sizeof(int) + alignof(float);
            return sz;
        }
    ";
    let mut parser = Parser::new(source, "test.c");
    let _program = parser.parse_program();
    assert!(parser.errors.is_empty());
}

#[test]
fn test_parser_types() {
    let source = "
        unsigned int u = 10;
        unsigned char uc = 'a';
        unsigned short us = 5;
        unsigned long ul = 20;
        signed int si = -1;
        _Atomic(int) atom = 0;
        typeof(int) t1 = 5;
        typeof(u) t2 = 10;
    ";
    let mut parser = Parser::new(source, "test.c");
    let _program = parser.parse_program();
    assert!(parser.errors.is_empty());
}

#[test]
fn test_parser_error_recovery() {
    // Parser should recover from syntax error (missing semicolon) and parse next declaration
    let source = "
        int x = 5
        struct Point { int x; int y; };
    ";
    let mut parser = Parser::new(source, "test.c");
    let program = parser.parse_program();
    assert!(!parser.errors.is_empty());
    // Should successfully parse structural declaration after recovering
    assert_eq!(program.decls.len(), 1);
}
