# Task: Add arithmetic shifts and range proofs

- **ID**: `US-003-TASK-003`
- **Story ID**: `US-003`
- **Status**: `PENDING`
- **Change Type**: `FEATURE`
- **Depends On**: `US-003-TASK-002`
- **Target Files**: `stricc/src/typechecker.rs`, `stricc/src/ast.rs`, `stricc/src/error.rs`, `stricc/tests/unit/test_typechecker.rs`, `stricc/tests/integration/type_safety.rs`, `stricc/tests/black_box/type_safety_cli.rs`, `stricc/tests/fixtures/us003/`

## Description

Implement explicit 32-bit arithmetic results with ProvenSafe, CompileError, and NeedsRuntimeCheck states. Check overflow, division, unknown divisors, floating operations, masked shifts, index ranges, pointer offsets, loop bounds, and conservative fixed-point merges. Preserve runtime-check requirements for unknown facts. Add range facts and in-memory proof doubles. Add unit, integration, and black-box tests for every scenario, boundary, fixture, repeated run, and optimization-independent result. Estimate 8 CU.
