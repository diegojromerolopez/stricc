# Task: Create check command walking skeleton

- **ID**: `US-003-TASK-001`
- **Story ID**: `US-003`
- **Status**: `PENDING`
- **Change Type**: `FEATURE`
- **Depends On**: `US-002-TASK-001`
- **Target Files**: `stricc/src/driver.rs`, `stricc/src/main.rs`, `stricc/src/error.rs`, `stricc/tests/unit/test_error.rs`, `stricc/tests/integration/type_safety.rs`, `stricc/tests/black_box/type_safety_cli.rs`

## Description

Implement the thin CLI shell and testable driver entry point for --check. Validate one input path, file errors, directories, and extra arguments. Define stable diagnostic categories, locations, prefixes, and exit codes. Preserve parser behavior from US-002. Add in-memory source and diagnostic doubles. Add smoke, error, integration, and black-box tests for command behavior. Estimate 5 CU.
