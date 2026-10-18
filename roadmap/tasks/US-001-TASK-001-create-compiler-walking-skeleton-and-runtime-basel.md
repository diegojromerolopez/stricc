# Task: Create compiler walking skeleton and runtime baseline

- **ID**: `US-001-TASK-001`
- **Story ID**: `US-001`
- **Status**: `PENDING`
- **Change Type**: `FEATURE`
- **Depends On**: `[]` 
- **Target Files**: `stricc/src/main.rs`, `stricc/src/lib.rs`, `stricc/src/driver.rs`, `stricc/src/error.rs`, `runtime/src/lib.rs`, `stricc/tests/support/mod.rs`, `stricc/tests/fixtures/characterization/smoke.c`, `stricc/tests/fixtures/characterization/README.md`, `stricc/tests/unit/test_error.rs`

## Description

Implement the thin CLI wrapper in main.rs with fewer than 15 lines. Add library entry points in lib.rs and driver.rs. Enforce '#![deny(unsafe_code)]' in all Rust crate roots. Implement diagnostics with 'error:' prefixes in error.rs. Add test doubles in support/mod.rs: InMemorySourceStore, InMemoryArtifactStore, RecordingDiagnostics, FixedTargetConfig, and RecordingStage. Implement safe runtime hooks in runtime/src/lib.rs. Create smoke test fixture smoke.c and README.md. Write unit tests in test_error.rs to verify error prefixes and exit codes.
