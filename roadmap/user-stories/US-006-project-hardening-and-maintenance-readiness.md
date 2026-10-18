# US-004: Project Hardening, Maintenance Readiness and Code Quality

## Story

As a maintainer, I want a clean, tested, packaged, and documented repository so that I can release `stricc` with confidence.

## Scope

Harden the complete repository after US-001 through US-005. Apply formatting and import hygiene. Run static analysis, unit tests, integration tests, GCC tests, LLVM tests, packaging checks, documentation checks, and black-box acceptance tests.

Treat existing code as legacy code. Preserve approved behavior unless a predecessor story changes the contract. Record each failure with its command, path, and diagnostic output.

## Depends On

- US-001
- US-002
- US-003
- US-004
- US-005

## Complexity

28 CU

## Observable Entry Points

Use these executable entry points from the repository root:

- `cargo fmt --all -- --check`
- `cargo clippy --workspace --all-targets --all-features -- -D warnings`
- `cargo test --workspace --all-targets`
- `cargo build --workspace`
- `make test-gcc`
- `make test-llvm`
- `python3 stricc/tests/gcc_torture_runner.py`
- `python3 stricc/tests/llvm_test_suite_runner.py`
- `cargo run -p stricc -- <options> <input.c>`
- `target/debug/stricc <options> <input.c>`
- `cargo package -p stricc --allow-dirty`
- `cargo doc --workspace --no-deps`

Do not use `|| true`, output redirection that hides errors, or silent command failure. Return a non-zero status for every failed check.

The `stricc` binary must accept the public CLI contract from US-002 and US-005. It must emit compiler output on standard output and diagnostics on standard error. It must return status `0` for successful compilation and a non-zero status for every rejected input or failed operation.

## Required Repository Paths

Keep these paths modular and independently testable:

- `stricc/src/main.rs` — thin CLI wrapper.
- `stricc/src/lib.rs` — public compiler library exports.
- `stricc/src/driver.rs` — compilation workflow.
- `stricc/src/lexer.rs` — tokenization.
- `stricc/src/parser.rs` — parsing.
- `stricc/src/ast.rs` — syntax data types.
- `stricc/src/symbol_table.rs` — symbol storage.
- `stricc/src/typechecker.rs` — semantic checks and value-range analysis.
- `stricc/src/codegen.rs` — LLVM generation.
- `stricc/src/error.rs` — diagnostic types and formatting.
- `runtime/src/lib.rs` — safe runtime support.
- `stricc/build.rs` — build integration.
- `stricc/tests/runner.rs` — integration test runner.
- `stricc/tests/gcc_torture_runner.py` — GCC matrix runner.
- `stricc/tests/llvm_test_suite_runner.py` — LLVM matrix runner.
- `stricc/tests/unit/test_codegen.rs` — code generation tests.
- `stricc/tests/unit/test_error.rs` — diagnostic tests.
- `stricc/tests/unit/test_lexer.rs` — lexer tests.
- `stricc/tests/unit/test_parser.rs` — parser tests.
- `stricc/tests/unit/test_typechecker.rs` — type checker tests.
- `stricc/tests/defined/` — defined-behavior cases.
- `stricc/tests/safety/` — safety rejection and trap cases.
- `docs/conf.py` — documentation configuration.
- `Cargo.toml` — workspace and package metadata.
- `Cargo.lock` — locked dependency versions.
- `Makefile` — verification targets.

Do not make internal modules import from `stricc::lib` or another parent facade. Import direct sibling modules or explicit shared types. Keep test doubles in test modules or test support modules.

## Quality Requirements

1. Run `cargo fmt --all -- --check` with no formatting difference.
2. Run Clippy with `-D warnings` and report zero warnings.
3. Place `#![deny(unsafe_code)]` in every Rust crate root.
4. Keep zero `unsafe` blocks and zero `unsafe` functions in compiler and runtime Rust code.
5. Run all workspace unit and integration tests with zero failures.
6. Run the complete defined-behavior matrix with zero failures.
7. Run the complete safety matrix with expected rejection or trap results.
8. Run GCC verification with a 100 percent pass rate.
9. Run LLVM verification with a 100 percent pass rate.
10. Build `libstricc_rt.a` and the `stricc` binary from a clean workspace.
11. Verify package metadata, included files, and package compilation.
12. Build documentation with no warnings or broken links.
13. Run every black-box CLI scenario from US-005.
14. Preserve deterministic diagnostics, exit codes, generated LLVM, and runtime trap behavior.
15. Reject stubs and placeholders, including `pass`, `...`, `NotImplementedError`, empty success branches, and fake return values.

## Output and Diagnostic Contract

Use exact prefixes for compiler diagnostics:

- Syntax error: `stricc: error: syntax: `
- Semantic error: `stricc: error: semantic: `
- CLI error: `stricc: error: cli: `
- Input error: `stricc: error: input: `
- Runtime trap: `stricc: runtime trap: `

Write normal compilation status only to standard output. Write all errors and traps to standard error. End each diagnostic with one newline. Do not print timestamps, memory addresses, random identifiers, or platform-dependent wording.

Use these verification markers in repository verification scripts:

- Success: `stricc-check: PASS <check-name>`
- Failure: `stricc-check: FAIL <check-name>`

A successful command may print compiler or test details, but it must include its required `PASS` marker. A failed command must include its `FAIL` marker and return a non-zero status.

Represent C `int` values as signed 32-bit values. Represent `float` values as IEEE-754 binary32 and `double` values as IEEE-754 binary64. Apply the deterministic overflow, shift, division, pointer, and trap rules defined by US-003, US-004, and US-005.

## Test Design Requirements

Use Chicago-school tests. Test observable behavior through real module boundaries. Use in-memory test doubles for file systems, diagnostic sinks, target selection, process execution, and linker results where a unit or integration test needs an external dependency. Do not use network services or real system mutation in these tests.

Run black-box tests through the public binary. Cover success, invalid options, missing input, empty input, malformed source, unsupported syntax, boundary integers, shift counts `0`, `31`, `32`, and `35`, division by zero, signed division overflow, null access, out-of-bounds access, use-after-free, double free, uninitialized reads, and non-void fallthrough.

Poll for process readiness when a test uses a process boundary. Do not use fixed sleep delays. This story adds no time, TTL, or expiration feature, so no clock is required.

## Acceptance Matrix

| Area | Required command or case | Expected result |
|---|---|---|
| Formatting | `cargo fmt --all -- --check` | Status `0`; no diff |
| Static analysis | `cargo clippy --workspace --all-targets --all-features -- -D warnings` | Status `0`; zero warnings |
| Rust tests | `cargo test --workspace --all-targets` | Status `0`; zero failures |
| Build | `cargo build --workspace` | Status `0`; all artifacts exist |
| GCC | `make test-gcc` | Status `0`; 100 percent pass |
| LLVM | `make test-llvm` | Status `0`; 100 percent pass |
| Packaging | `cargo package -p stricc --allow-dirty` | Status `0`; package builds |
| Documentation | `cargo doc --workspace --no-deps` | Status `0`; no warnings |
| Defined cases | Every file under `stricc/tests/defined/` | Expected result and output match |
| Safety cases | Every file under `stricc/tests/safety/` | Expected rejection or trap matches |
| Black box | Every US-005 CLI scenario | Exact status and diagnostic contract matches |

Do not reduce the matrix by deleting cases, changing expected results without evidence, or excluding failed targets. Update characterization tests only when the predecessor contract requires the change.

## Definition of Done

- [ ] Apply repository-wide formatting and import hygiene.
- [ ] Verify `#![deny(unsafe_code)]` in every Rust crate root.
- [ ] Verify zero forbidden unsafe code and zero stubs.
- [ ] Pass Clippy with `-D warnings`.
- [ ] Pass all unit and integration tests.
- [ ] Pass all defined and safety cases.
- [ ] Pass GCC and LLVM verification at 100 percent.
- [ ] Pass packaging and documentation checks.
- [ ] Pass the complete black-box acceptance matrix.
- [ ] Confirm exact exit statuses and diagnostic prefixes.
- [ ] Confirm deterministic output across two clean repeated runs.
- [ ] Review all changed files and remove temporary artifacts.

## Verification Criteria

The story is complete only when every command in the Observable Entry Points section runs from a clean checkout and returns the required status. The verification report must list each command, status, pass marker, compiler version, LLVM version, Rust version, and changed test count.

Run the full acceptance matrix twice. Compare compiler diagnostics, LLVM text, exit statuses, and test summaries. Reject the change if any result differs without an approved target-specific reason.

## Machine-Readable Contract

```noctifab-contract
{
  "story_id": "US-004",
  "public_contracts": [
    {
      "id": "US-006-verification-cli",
      "interface": "repository-root verification commands and target/debug/stricc public compiler CLI",
      "allowed_executables": [
        "cargo",
        "make",
        "python3",
        "target/debug/stricc"
      ],
      "exit_codes": {
        "success": [0],
        "error": "non-zero"
      },
      "stdout_contains": [
        "stricc-check: PASS"
      ],
      "stderr_prefixes": [
        "stricc: error: syntax: ",
        "stricc: error: semantic: ",
        "stricc: error: cli: ",
        "stricc: error: input: ",
        "stricc: runtime trap: "
      ]
    }
  ]
}
```

## Change Metadata

- `depends_on`: ["US-001", "US-002", "US-003", "US-004", "US-005"]
- `change_type`: `new`
