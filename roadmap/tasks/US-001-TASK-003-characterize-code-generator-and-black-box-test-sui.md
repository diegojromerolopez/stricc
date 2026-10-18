# Task: Characterize code generator and black box test suite

- **ID**: `US-001-TASK-003`
- **Story ID**: `US-001`
- **Status**: `PENDING`
- **Change Type**: `FEATURE`
- **Depends On**: `US-001-TASK-002`
- **Target Files**: `stricc/src/codegen.rs`, `stricc/tests/unit/test_codegen.rs`, `stricc/tests/characterization.rs`, `stricc/tests/black_box.rs`, `stricc/tests/runner.rs`, `stricc/tests/gcc_torture_runner.py`, `stricc/tests/llvm_test_suite_runner.py`

## Description

Implement deterministic LLVM code emission in codegen.rs. Support minimal functions, arithmetic operations, and conditional branches. Write unit characterization tests in test_codegen.rs. Implement component integration characterization tests in characterization.rs. Add black-box scenarios in black_box.rs to verify CLI exit status and output files. Configure existing test runners in runner.rs, gcc_torture_runner.py, and llvm_test_suite_runner.py. Verify target/debug/stricc creates smoke.ll from smoke.c with zero exit status.
