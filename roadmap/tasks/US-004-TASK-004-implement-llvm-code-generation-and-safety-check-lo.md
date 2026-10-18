# Task: Implement LLVM Code Generation and Safety Check Lowering

- **ID**: `US-004-TASK-004`
- **Story ID**: `US-004`
- **Status**: `PENDING`
- **Change Type**: `FEATURE`
- **Depends On**: `US-004-TASK-001`, `US-004-TASK-002`, `US-004-TASK-003`
- **Target Files**: `stricc/src/codegen.rs`, `stricc/src/lib.rs`, `stricc/tests/unit/test_codegen.rs`, `stricc/tests/integration/codegen_pipeline.rs`

## Description

Lower typed AST statements and expressions into LLVM 18 IR. Emit safe runtime checks for unproven memory operations and omit proven checks.
