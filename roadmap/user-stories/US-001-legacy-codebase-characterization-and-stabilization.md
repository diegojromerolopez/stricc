# US-001: Legacy Codebase Characterization & Stabilization

## Story

As a compiler maintainer, I want a tested baseline for the legacy compiler, so that later changes preserve defined behavior.

## Scope

Characterize the existing compiler before structural refactoring. Cover the lexer, parser, AST, symbol table, type checker, code generator, driver, CLI, runtime, and test harness. Create one repeatable compiler smoke path. Record current behavior in tests and fixtures.

Do not add new language behavior in this story. Preserve behavior unless a test exposes a crash, data loss, non-deterministic result, or unsafe Rust code.

## Dependencies

- `depends_on: []`
- `change_type: "new"`
- Complexity: 30 CU
- Domain capability: Legacy Compiler Baseline and Walking Skeleton

## Repository Layout

Use these exact repository paths:

- `stricc/src/main.rs`: thin CLI wrapper.
- `stricc/src/lib.rs`: public compiler modules and callable entry points.
- `stricc/src/driver.rs`: compilation workflow.
- `stricc/src/lexer.rs`: tokenization.
- `stricc/src/parser.rs`: parsing.
- `stricc/src/ast.rs`: syntax data types.
- `stricc/src/symbol_table.rs`: symbol state.
- `stricc/src/typechecker.rs`: semantic checks.
- `stricc/src/codegen.rs`: LLVM generation.
- `stricc/src/error.rs`: diagnostic values and rendering.
- `runtime/src/lib.rs`: runtime support.
- `stricc/tests/unit/test_lexer.rs`: lexer characterization tests.
- `stricc/tests/unit/test_parser.rs`: parser characterization tests.
- `stricc/tests/unit/test_typechecker.rs`: type checker characterization tests.
- `stricc/tests/unit/test_codegen.rs`: code generator characterization tests.
- `stricc/tests/unit/test_error.rs`: diagnostic characterization tests.
- `stricc/tests/characterization.rs`: component integration tests.
- `stricc/tests/black_box.rs`: CLI black-box tests.
- `stricc/tests/support/mod.rs`: in-memory doubles and test helpers.
- `stricc/tests/fixtures/characterization/`: stable source inputs and expected results.
- `stricc/tests/fixtures/characterization/README.md`: fixture rules.
- `stricc/tests/gcc_torture_runner.py`: existing GCC runner.
- `stricc/tests/llvm_test_suite_runner.py`: existing LLVM runner.
- `stricc/tests/runner.rs`: existing test runner.

Keep component code in its own module. Do not make internal modules import parent facades or index modules. Use direct module paths for internal imports.

## Public Entry Point and CLI Contract

Build the compiler with `cargo build -p stricc`.

Run the compiler with `cargo run -p stricc -- <options> <input>`.

Use the binary at `target/debug/stricc` after the build. The smoke command is:

`target/debug/stricc --emit-llvm --output target/characterization/smoke.ll stricc/tests/fixtures/characterization/smoke.c`

Support the options already accepted by the legacy CLI. Characterize each accepted option. Do not silently remove an option. Reject an unknown option with a non-zero exit status.

Use one input path per command. Read source from that path. Write the requested artifact to the `--output` path. Do not overwrite the input file.

For successful compilation, return exit status `0`. Write diagnostics to stderr. Keep stdout empty unless the legacy command already writes a documented result there.

For invalid input, invalid options, missing input, unreadable input, or output failure, return a non-zero exit status. Start each diagnostic with `error:`. Keep the diagnostic stable after the baseline is recorded.

The smoke path must perform the existing driver stages in order: input read, preprocessing if configured, lexing, parsing, type checking, LLVM emission, and output write. Record the stages in a testable result or capture their observable output. Do not bypass a stage with a fake artifact.

The compiler must not expose a network protocol. No socket, port, HTTP, RESP, or wire server exists in this story. Do not add one.

## Baseline Rules

1. Inspect every legacy source file before changing it.
2. Run the existing unit, integration, and black-box tests before adding tests.
3. Record current behavior for valid and invalid inputs.
4. Preserve token spans, AST equality, diagnostic text, exit status, and generated LLVM text where tests already define them.
5. Add a characterization test before changing behavior.
6. Make output deterministic across two identical runs.
7. Sort unordered diagnostic or symbol output before comparison.
8. Use explicit `i32`, `f32`, `f64`, `u8`, `bool`, and `usize` meanings where tests inspect values.
9. Treat integer boundaries as exact values, not host-dependent values.
10. Do not claim safety or language support that the legacy code does not provide.

## Required Test Doubles

Use Chicago-school tests. Test real module behavior through public callable functions. Use in-memory doubles where an external boundary exists.

Implement these doubles in `stricc/tests/support/mod.rs`:

- `InMemorySourceStore` for source reads.
- `InMemoryArtifactStore` for generated output.
- `RecordingDiagnostics` for stdout and stderr capture.
- `FixedTargetConfig` for target and optimization settings.
- `RecordingStage` for driver stage order.

Use no disk, process, network, or clock dependency in component tests. Use real parser, type checker, and code generator components with these doubles. Use temporary directories only in black-box tests.

No feature in this story uses time, expiration, or TTL. Do not add a clock. If a future test introduces time, use a deterministic mock clock and no sleep delay.

## Characterization Matrix

| Area | Input or condition | Required observation |
|---|---|---|
| Lexer | Empty source | Record token stream and success or error result. |
| Lexer | Whitespace and comments | Record skipped text and source positions. |
| Lexer | Identifiers and keywords | Record exact token kinds and lexemes. |
| Lexer | Decimal integer and floating literals | Record exact value representation. |
| Lexer | Operators and punctuation | Record token order and spans. |
| Lexer | Unterminated or invalid token | Record stable `error:` diagnostic. |
| Parser | Empty translation unit | Record AST result. |
| Parser | Function declaration and return | Record AST structure. |
| Parser | Nested expressions | Record precedence and associativity. |
| Parser | Arrays, pointers, and member access | Record AST structure and source spans. |
| Parser | Missing delimiter | Record error position and message. |
| Symbol table | Insert, lookup, shadow, and duplicate name | Record result and diagnostic. |
| Type checker | Valid primitive expressions | Record inferred or checked type. |
| Type checker | Invalid operand types | Record stable diagnostic. |
| Type checker | Missing return and uninitialized read | Record current legacy result. |
| Code generator | Minimal valid function | Record valid LLVM text or stable failure. |
| Code generator | Arithmetic and branch expression | Record instruction shape and result. |
| Driver | Valid source and output path | Record stage order and artifact. |
| Driver | Missing source | Record non-zero status and `error:` prefix. |
| CLI | Unknown option | Record non-zero status and stable diagnostic. |
| CLI | Missing option value | Record non-zero status and stable diagnostic. |
| Runtime | Existing exported function behavior | Record return or abort behavior without unsafe test code. |
| Harness | Repeated test execution | Record identical results and no leaked state. |

Add a fixture for every matrix row. Use small source files. Keep expected output beside each fixture. Explain each fixture in `stricc/tests/fixtures/characterization/README.md`.

## Black-Box Scenarios

Run each scenario against `target/debug/stricc` without importing compiler internals:

1. Compile the smallest valid source.
2. Compile a function with an integer return.
3. Emit LLVM to a new output file.
4. Compile with each legacy accepted option.
5. Reject an unknown option.
6. Reject a missing option value.
7. Reject a missing input path.
8. Reject an unreadable input path.
9. Reject malformed source.
10. Reject an output path that cannot be written.
11. Run the same valid command twice and compare status, stdout, stderr, and artifact bytes.
12. Run the same invalid command twice and compare status and diagnostics.
13. Run the existing defined fixtures that the legacy driver supports.
14. Run the existing safety fixtures and record whether each rejects, traps, or remains unsupported.

Do not reinterpret an existing fixture as a new requirement. Mark unsupported behavior as a baseline result and link it to a later story.

Use readiness polling only when a future test starts a service. This story starts no service and must use no static sleep delay.

## Exact Output Invariants

- Successful commands use exit status `0`.
- Failed commands use a non-zero exit status.
- Error diagnostics begin with `error:`.
- Warnings begin with `warning:` if the legacy compiler emits warnings.
- Diagnostics use UTF-8 text and a trailing newline.
- Line and column values use one-based numbering when present.
- Integer literal tests use signed 32-bit values where the AST declares `i32`.
- Float tests use IEEE-754 single precision where the AST declares `f32`.
- Double tests use IEEE-754 double precision where the AST declares `f64`.
- Do not compare pointer addresses, temporary paths, timestamps, or process identifiers.
- Normalize only values that the test names as non-deterministic. Never hide a compiler error.

## Anti-Stub and Safety Constraints

Do not add `pass`, `...`, `TODO` in executable paths, `NotImplementedError`, empty success branches, fake LLVM output, or placeholder return values. Do not use `|| true`, ignored shell errors, or broad error suppression. A test must fail when the required behavior fails.

Keep `#![deny(unsafe_code)]` in every Rust crate root. Keep zero unsafe blocks in compiler and runtime Rust code. Do not add raw-pointer dereferences, unsafe FFI implementations, or unsupported ABI claims. Do not weaken lint settings to pass the baseline.

Keep `stricc/src/main.rs` as a thin wrapper with fewer than 15 non-empty lines. Delegate immediately to a callable function in `stricc/src/lib.rs` or `stricc/src/driver.rs`. Put argument parsing and workflow logic outside `main`.

Do not perform a broad refactor in this story. Do not implement the full C23 subset, VRP, LLVM 18 safety traps, shadow metadata, or new runtime allocation rules. Those changes belong to later stories.

## Verification Commands

Run these commands from the repository root:

- `cargo fmt --all -- --check`
- `cargo check --workspace --all-targets`
- `cargo test --workspace --all-targets`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `cargo build -p stricc`
- `target/debug/stricc --emit-llvm --output target/characterization/smoke.ll stricc/tests/fixtures/characterization/smoke.c`
- `python3 stricc/tests/gcc_torture_runner.py`
- `python3 stricc/tests/llvm_test_suite_runner.py`

Use the repository's existing runner arguments when a runner requires them. Report a command failure. Do not mask it.

## Acceptance Criteria

- [ ] Inspect and document all legacy compiler modules named in this story.
- [ ] Add characterization coverage for lexer, parser, symbol table, type checker, code generator, driver, CLI, runtime, and harness behavior.
- [ ] Add the required in-memory doubles and use them in component tests.
- [ ] Add the required black-box scenarios.
- [ ] Build and run the smoke command successfully for the baseline fixture.
- [ ] Preserve exact baseline results in versioned fixtures.
- [ ] Produce identical results for repeated valid and invalid commands.
- [ ] Return `0` only for successful commands.
- [ ] Return non-zero for every tested error condition.
- [ ] Keep error diagnostics prefixed with `error:`.
- [ ] Keep `main.rs` below 15 non-empty lines and delegate immediately.
- [ ] Keep zero unsafe Rust code and pass `#![deny(unsafe_code)]`.
- [ ] Add no stubs, placeholders, ignored failures, or shell error masking.
- [ ] Pass every verification command with zero test failures.

## Definition of Done

The story is complete only when all acceptance criteria pass. Unit tests, integration tests, and black-box tests must pass at 100 percent. The workspace must report zero test failures, zero clippy warnings, and a clean format check. The smoke artifact must exist at the requested output path. The final change must contain no unrecorded behavior change.

```noctifab-contract
{
  "story_id": "US-001",
  "public_contracts": [
    {
      "id": "compiler-cli-baseline",
      "interface": "target/debug/stricc --emit-llvm --output <path> <input>",
      "allowed_executables": ["cargo", "target/debug/stricc", "python3"],
      "exit_codes": {"success": [0], "error": "non-zero"},
      "stdout_contains": [],
      "stderr_prefixes": ["error:", "warning:"]
    },
    {
      "id": "smoke-artifact",
      "interface": "LLVM artifact written to the exact --output path",
      "allowed_executables": ["target/debug/stricc"],
      "exit_codes": {"success": [0], "error": "non-zero"},
      "stdout_contains": [],
      "stderr_prefixes": ["error:"]
    }
  ]
}
```