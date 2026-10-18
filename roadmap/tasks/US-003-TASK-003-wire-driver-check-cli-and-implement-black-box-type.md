# Task: Wire driver check CLI and implement black-box type safety test suite

- **ID**: `US-003-TASK-003`
- **Story ID**: `US-003`
- **Status**: `PENDING`
- **Change Type**: `FEATURE`
- **Depends On**: `US-003-TASK-002`
- **Target Files**: `stricc/src/driver.rs`, `stricc/src/main.rs`, `stricc/tests/black_box/type_safety_cli.rs`, `stricc/tests/fixtures/us003/valid_vrp.c`, `stricc/tests/fixtures/us003/invalid_div_zero.c`, `stricc/tests/fixtures/us003/invalid_lifetime.c`, `stricc/tests/fixtures/us003/invalid_bounds.c`

## Description

Connect driver check workflow to semantic analysis. Format diagnostics to standard error and enforce exit codes. Add C test fixtures. Create black-box CLI test suites covering all scenarios.
