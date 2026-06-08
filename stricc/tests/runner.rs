use std::process::Command;
use std::fs;
use std::path::{Path, PathBuf};

struct TestCase {
    name: &'static str,
    file_path: &'static str,
    expected_error: Option<&'static str>, // If Some, compilation must fail and output contain this
    expected_abort: Option<&'static str>, // If Some, binary must abort and stderr contain this
}

fn get_workspace_root() -> PathBuf {
    if let Ok(dir) = std::env::var("CARGO_MANIFEST_DIR") {
        Path::new(&dir).parent().unwrap().to_path_buf()
    } else {
        PathBuf::from("/Users/diegoj/repos/stricc")
    }
}

fn build_compiler() {
    let status = Command::new("cargo")
        .arg("build")
        .arg("--workspace")
        .current_dir(get_workspace_root())
        .status()
        .expect("Failed to run cargo build");
    assert!(status.success(), "Workspace build failed");
}

fn run_test_case(case: &TestCase) {
    let workspace_root = get_workspace_root();
    let stricc_bin = workspace_root.join("target/debug/stricc");
    let test_file = workspace_root.join(case.file_path);
    let output_bin = workspace_root.join(format!("target/debug/test_{}", case.name));

    // Ensure output target folder exists
    let _ = fs::create_dir_all(output_bin.parent().unwrap());

    // Remove old output binary if exists
    if output_bin.exists() {
        let _ = fs::remove_file(&output_bin);
    }

    // Compile with stricc
    let mut cmd = Command::new(&stricc_bin);
    cmd.arg("-o")
        .arg(&output_bin)
        .arg(&test_file);

    let compile_output = cmd.output().expect("Failed to execute stricc compiler");

    if let Some(err_msg) = case.expected_error {
        // Compilation must fail
        assert!(
            !compile_output.status.success(),
            "Expected compilation failure for test '{}' but it succeeded",
            case.name
        );
        let stderr = String::from_utf8_lossy(&compile_output.stderr);
        assert!(
            stderr.contains(err_msg),
            "Expected compiler error containing '{}', but got:\n{}",
            err_msg,
            stderr
        );
    } else {
        // Compilation must succeed
        assert!(
            compile_output.status.success(),
            "Expected compilation success for test '{}', but it failed with:\n{}",
            case.name,
            String::from_utf8_lossy(&compile_output.stderr)
        );

        // Run the output binary and check for expected runtime abort
        let run_output = Command::new(&output_bin)
            .output()
            .expect("Failed to run compiled test binary");

        if let Some(abort_msg) = case.expected_abort {
            assert!(
                !run_output.status.success(),
                "Expected runtime abort for test '{}' but it exited cleanly",
                case.name
            );
            let stderr = String::from_utf8_lossy(&run_output.stderr);
            assert!(
                stderr.contains(abort_msg),
                "Expected runtime abort containing '{}', but got:\n{}",
                abort_msg,
                stderr
            );
        } else {
            // Must exit cleanly
            assert!(
                run_output.status.success(),
                "Expected clean exit for test '{}', but it failed with code {:?} and stderr:\n{}",
                case.name,
                run_output.status.code(),
                String::from_utf8_lossy(&run_output.stderr)
            );
        }
    }
}

#[test]
fn test_safety_matrix() {
    build_compiler();

    let test_cases = vec![
        TestCase {
            name: "div_zero",
            file_path: "stricc/tests/safety/div_zero.c",
            expected_error: None,
            expected_abort: Some("Division by zero"),
        },
        TestCase {
            name: "overflow",
            file_path: "stricc/tests/safety/overflow.c",
            expected_error: None,
            expected_abort: Some("Integer overflow detected"),
        },
        TestCase {
            name: "overflow_sub",
            file_path: "stricc/tests/safety/overflow_sub.c",
            expected_error: None,
            expected_abort: Some("Integer overflow detected"),
        },
        TestCase {
            name: "overflow_mul",
            file_path: "stricc/tests/safety/overflow_mul.c",
            expected_error: None,
            expected_abort: Some("Integer overflow detected"),
        },
        TestCase {
            name: "bounds",
            file_path: "stricc/tests/safety/bounds.c",
            expected_error: None,
            expected_abort: Some("Out-of-bounds pointer access"),
        },
        TestCase {
            name: "null_deref",
            file_path: "stricc/tests/safety/null_deref.c",
            expected_error: None,
            expected_abort: Some("Out-of-bounds pointer access"),
        },
        TestCase {
            name: "uaf",
            file_path: "stricc/tests/safety/uaf.c",
            expected_error: None,
            expected_abort: Some("Use-after-free detected"),
        },
        TestCase {
            name: "double_free",
            file_path: "stricc/tests/safety/double_free.c",
            expected_error: None,
            expected_abort: Some("Double-free or invalid free"),
        },
        TestCase {
            name: "escape",
            file_path: "stricc/tests/safety/escape.c",
            expected_error: Some("Address of local variable escapes"),
            expected_abort: None,
        },
        TestCase {
            name: "noreturn",
            file_path: "stricc/tests/safety/noreturn.c",
            expected_error: Some("does not return a value on all control flow paths"),
            expected_abort: None,
        },
        TestCase {
            name: "float_overflow",
            file_path: "stricc/tests/safety/float_overflow.c",
            expected_error: None,
            expected_abort: Some("Floating point overflow or NaN"),
        },
        TestCase {
            name: "uninit",
            file_path: "stricc/tests/safety/uninit.c",
            expected_error: None,
            expected_abort: Some("Division by zero"),
        },
        TestCase {
            name: "unaligned",
            file_path: "stricc/tests/safety/unaligned.c",
            expected_error: None,
            expected_abort: Some("Unaligned memory access"),
        },
        TestCase {
            name: "float_to_int",
            file_path: "stricc/tests/safety/float_to_int.c",
            expected_error: None,
            expected_abort: Some("Float-to-int conversion overflow"),
        },
        TestCase {
            name: "div_overflow",
            file_path: "stricc/tests/safety/div_overflow.c",
            expected_error: None,
            expected_abort: Some("Division overflow"),
        },
    ];

    for case in &test_cases {
        println!("Running integration test: {}", case.name);
        run_test_case(case);
    }
}

#[test]
fn test_defined_behavior() {
    build_compiler();

    let test_cases = vec![
        TestCase {
            name: "uninitialized_read",
            file_path: "stricc/tests/defined/uninitialized_read.c",
            expected_error: None,
            expected_abort: None,
        },
        TestCase {
            name: "shift_mask",
            file_path: "stricc/tests/defined/shift_mask.c",
            expected_error: None,
            expected_abort: None,
        },
        TestCase {
            name: "shift_constant_error",
            file_path: "stricc/tests/defined/shift_constant_error.c",
            expected_error: Some("Shift count 35 is out of bounds for type Int"),
            expected_abort: None,
        },
        TestCase {
            name: "wrap_overflow",
            file_path: "stricc/tests/defined/wrap_overflow.c",
            expected_error: None,
            expected_abort: None,
        },
        TestCase {
            name: "overlap_memcpy",
            file_path: "stricc/tests/defined/overlap_memcpy.c",
            expected_error: None,
            expected_abort: None,
        },
        TestCase {
            name: "null_memcpy_zero",
            file_path: "stricc/tests/defined/null_memcpy_zero.c",
            expected_error: None,
            expected_abort: None,
        },
        TestCase {
            name: "pointer_compare",
            file_path: "stricc/tests/defined/pointer_compare.c",
            expected_error: None,
            expected_abort: None,
        },
        TestCase {
            name: "cfi_mismatch",
            file_path: "stricc/tests/defined/cfi_mismatch.c",
            expected_error: None,
            expected_abort: Some("CFI violation"),
        },
        TestCase {
            name: "string_bounds",
            file_path: "stricc/tests/defined/string_bounds.c",
            expected_error: None,
            expected_abort: Some("String not null-terminated within bounds in strlen"),
        },
        TestCase {
            name: "ctype_range",
            file_path: "stricc/tests/defined/ctype_range.c",
            expected_error: None,
            expected_abort: Some("ctype.h argument out of range"),
        },
        TestCase {
            name: "invalid_cast",
            file_path: "stricc/tests/defined/invalid_cast.c",
            expected_error: Some("Const qualifier discarded"),
            expected_abort: None,
        },
        TestCase {
            name: "format_mismatch",
            file_path: "stricc/tests/defined/format_mismatch.c",
            expected_error: Some("Format specifier %s expects string pointer"),
            expected_abort: None,
        },
        TestCase {
            name: "link_mismatch",
            file_path: "stricc/tests/defined/link_mismatch.c",
            expected_error: Some("Redeclaration of global variable"),
            expected_abort: None,
        },
        TestCase {
            name: "union_mismatch",
            file_path: "stricc/tests/defined/union_mismatch.c",
            expected_error: None,
            expected_abort: None,
        },
    ];

    for case in &test_cases {
        println!("Running defined behavior test: {}", case.name);
        run_test_case(case);
    }
}

