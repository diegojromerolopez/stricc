# Task: Implement type and lifetime analysis

- **ID**: `US-003-TASK-002`
- **Story ID**: `US-003`
- **Status**: `PENDING`
- **Change Type**: `FEATURE`
- **Depends On**: `US-003-TASK-001`, `US-002-TASK-001`
- **Target Files**: `stricc/src/symbol_table.rs`, `stricc/src/ast.rs`, `stricc/src/typechecker.rs`, `stricc/src/error.rs`, `stricc/tests/unit/test_typechecker.rs`, `stricc/tests/fixtures/us003/`

## Description

Implement typed symbols, deterministic scopes, declarations, fields, functions, and lifetime metadata. Add AST spans and semantic annotations when required. Check names, assignments, pointers, arrays, structs, calls, lvalues, const writes, returns, initialization, local escapes, and ended lifetimes. Merge branch and loop states conservatively. Add in-memory symbol tests and fixtures for type, initialization, lifetime, control, and access rules. Estimate 8 CU.
