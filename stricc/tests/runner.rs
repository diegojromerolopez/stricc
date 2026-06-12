use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

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

use std::sync::Once;

static BUILD_COMPILER_ONCE: Once = Once::new();

fn build_compiler() {
    BUILD_COMPILER_ONCE.call_once(|| {
        let status = Command::new("cargo")
            .arg("build")
            .arg("--workspace")
            .current_dir(get_workspace_root())
            .status()
            .expect("Failed to run cargo build");
        assert!(status.success(), "Workspace build failed");
    });
}

fn run_test_case(case: &TestCase) {
    let workspace_root = get_workspace_root();
    let stricc_bin = workspace_root.join("target/debug/stricc");
    let output_bin = workspace_root.join(format!("target/debug/test_{}", case.name));

    // Ensure target folder exists
    let _ = fs::create_dir_all(output_bin.parent().unwrap());

    // Clean up old output binary if any
    if output_bin.exists() {
        let _ = fs::remove_file(&output_bin);
    }

    // Compile with stricc
    let mut cmd = Command::new(&stricc_bin);
    cmd.arg("-o").arg(&output_bin);

    for path in case.file_path.split_whitespace() {
        cmd.arg(workspace_root.join(path));
    }

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
            "Expected compiler error containing '{err_msg}', but got:\n{stderr}"
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
                "Expected runtime abort containing '{abort_msg}', but got:\n{stderr}"
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
            name: "forbidden_redefine",
            file_path: "stricc/tests/safety/forbidden_redefine.c",
            expected_error: Some("Redefining keyword 'int' as a macro is forbidden in Safe C mode"),
            expected_abort: None,
        },
        TestCase {
            name: "stack_overflow",
            file_path: "stricc/tests/safety/stack_overflow.c",
            expected_error: None,
            expected_abort: Some("Stack overflow detected"),
        },
        TestCase {
            name: "aligned_alloc_invalid",
            file_path: "stricc/tests/safety/aligned_alloc_invalid.c",
            expected_error: None,
            expected_abort: Some("Invalid alignment or size in aligned_alloc"),
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
        TestCase {
            name: "vrp_invalid",
            file_path: "stricc/tests/safety/vrp_invalid.c",
            expected_error: Some("Static out-of-bounds array access"),
            expected_abort: None,
        },
        TestCase {
            name: "dynamic_format_invalid",
            file_path: "stricc/tests/safety/dynamic_format_invalid.c",
            expected_error: None,
            expected_abort: Some("stricc dynamic check failure: Mismatched printf argument type"),
        },
        TestCase {
            name: "seq_points_invalid",
            file_path: "stricc/tests/safety/seq_points_invalid.c",
            expected_error: Some("Sequence point violation: variable"),
            expected_abort: None,
        },
        TestCase {
            name: "write_to_const_invalid",
            file_path: "stricc/tests/safety/write_to_const_invalid.c",
            expected_error: None,
            expected_abort: Some("Attempted to write to a const object"),
        },
        TestCase {
            name: "ffi_sandbox_invalid",
            file_path: "stricc/tests/safety/ffi_sandbox_invalid_main.c stricc/tests/safety/ffi_sandbox_invalid_helper.c",
            expected_error: None,
            expected_abort: Some("stricc FFI sandbox violation"),
        },
        TestCase {
            name: "ffi_sandbox_page_invalid",
            file_path: "stricc/tests/safety/ffi_sandbox_page_invalid_main.c stricc/tests/safety/ffi_sandbox_page_invalid_helper.c",
            expected_error: None,
            expected_abort: Some("stricc FFI sandbox violation: Out-of-bounds read/write detected in third-party library call"),
        },
        TestCase {
            name: "stack_uaf_invalid",
            file_path: "stricc/tests/safety/stack_uaf_invalid.c",
            expected_error: None,
            expected_abort: Some("stricc dynamic check failure: Use-after-free detected"),
        },
        TestCase {
            name: "setjmp_invalid",
            file_path: "stricc/tests/safety/setjmp_invalid.c",
            expected_error: Some("setjmp/longjmp are forbidden"),
            expected_abort: None,
        },
        TestCase {
            name: "user_variadic_invalid",
            file_path: "stricc/tests/safety/user_variadic_invalid.c",
            expected_error: Some("User-defined variadic function"),
            expected_abort: None,
        },
        TestCase {
            name: "ptr_to_int_invalid",
            file_path: "stricc/tests/safety/ptr_to_int_invalid.c",
            expected_error: Some("Casting pointer to integer"),
            expected_abort: None,
        },
        TestCase {
            name: "vla_bounds_invalid",
            file_path: "stricc/tests/safety/vla_bounds_invalid.c",
            expected_error: None,
            expected_abort: Some("Out-of-bounds pointer access"),
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
            name: "aligned_alloc_valid",
            file_path: "stricc/tests/defined/aligned_alloc_valid.c",
            expected_error: None,
            expected_abort: None,
        },
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
        TestCase {
            name: "vrp_valid",
            file_path: "stricc/tests/defined/vrp_valid.c",
            expected_error: None,
            expected_abort: None,
        },
        TestCase {
            name: "link_type_invalid",
            file_path: "stricc/tests/defined/link_type_invalid_1.c stricc/tests/defined/link_type_invalid_2.c",
            expected_error: Some("__stricc_sig_var_my_global"),
            expected_abort: None,
        },
        TestCase {
            name: "link_type_valid",
            file_path: "stricc/tests/defined/link_type_valid_1.c stricc/tests/defined/link_type_valid_2.c",
            expected_error: None,
            expected_abort: None,
        },
        TestCase {
            name: "dynamic_format_valid",
            file_path: "stricc/tests/defined/dynamic_format_valid.c",
            expected_error: None,
            expected_abort: None,
        },
        TestCase {
            name: "seq_points_valid",
            file_path: "stricc/tests/defined/seq_points_valid.c",
            expected_error: None,
            expected_abort: None,
        },
        TestCase {
            name: "write_to_const_valid",
            file_path: "stricc/tests/defined/write_to_const_valid.c",
            expected_error: None,
            expected_abort: None,
        },
        TestCase {
            name: "ffi_sandbox_valid",
            file_path: "stricc/tests/defined/ffi_sandbox_valid_main.c stricc/tests/defined/ffi_sandbox_helper.c",
            expected_error: None,
            expected_abort: None,
        },
        TestCase {
            name: "shadow_prune_valid",
            file_path: "stricc/tests/defined/shadow_prune_valid.c",
            expected_error: None,
            expected_abort: None,
        },
        TestCase {
            name: "stack_uaf_valid",
            file_path: "stricc/tests/defined/stack_uaf_valid.c",
            expected_error: None,
            expected_abort: None,
        },
        TestCase {
            name: "infinite_loop_valid",
            file_path: "stricc/tests/defined/infinite_loop_valid.c",
            expected_error: None,
            expected_abort: None,
        },
        TestCase {
            name: "for_loop_valid",
            file_path: "stricc/tests/defined/for_loop_valid.c",
            expected_error: None,
            expected_abort: None,
        },
        TestCase {
            name: "vla_bounds_valid",
            file_path: "stricc/tests/defined/vla_bounds_valid.c",
            expected_error: None,
            expected_abort: None,
        },
        TestCase {
            name: "realloc_valid",
            file_path: "stricc/tests/defined/realloc_valid.c",
            expected_error: None,
            expected_abort: None,
        },
        TestCase {
            name: "memset_null_zero_valid",
            file_path: "stricc/tests/defined/memset_null_zero_valid.c",
            expected_error: None,
            expected_abort: None,
        },
        TestCase {
            name: "signed_shift_valid",
            file_path: "stricc/tests/defined/signed_shift_valid.c",
            expected_error: None,
            expected_abort: None,
        },
    ];

    for case in &test_cases {
        println!("Running defined behavior test: {}", case.name);
        run_test_case(case);
    }
}
