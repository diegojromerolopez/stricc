# Task: Implement LLVM Overflow and Division Trap Intrinsics

- **ID**: `US-004-TASK-003`
- **Story ID**: `US-004`
- **Status**: `PENDING`
- **Change Type**: `FEATURE`
- **Depends On**: `US-004-TASK-002`
- **Target Files**: `stricc/src/llvm_intrinsics.rs`, `stricc/tests/unit/test_llvm_intrinsics.rs`

## Description

Implement signed overflow arithmetic helpers and division guards. Apply shift masks and route arithmetic failures to deterministic runtime traps.
