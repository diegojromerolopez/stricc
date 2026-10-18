# US-003: Safe C Front End and Diagnostic Contracts

## 1. Story Metadata

- **ID**: US-002
- **Title**: Safe C Front End and Diagnostic Contracts
- **Slug**: US-002-safe-c-front-end-and-diagnostic-contracts
- **Domain Capability**: Lexing, Parsing, AST, Symbols, and CLI
- **Depends On**: [US-001]
- **Complexity**: 32 CU
- **Change Type**: new

## 2. Description

This user story implements the safe and deterministic C front end for the `stricc` compiler. You must implement the lexer, the parser, the abstract syntax tree (AST), the symbol table, and the command-line interface (CLI). You must parse the supported C subset. You must output deterministic diagnostics for valid input, invalid syntax, unsupported constructs, and invalid options.

## 3. Observable Entry Point and CLI Contracts

Use the compiler binary at `target/debug/stricc` or `target/release/stricc`.
Use `cargo run -p stricc --` as the development entry point. Put compiler arguments after `--`.

You must support these commands:
- `stricc --help`
- `stricc --version`
- `stricc --check <input.c>`
- `stricc --emit ast --check <input.c>`
- `stricc --emit tokens --check <input.c>`

You must support these options:
- `--help`: Prints help text and exits with status `0`.
- `--version`: Prints the tool version and exits with status `0`.
- `--check`: Runs lexing, parsing, and symbol collection.
- `--emit ast`: Prints the canonical AST after a successful check.
- `--emit tokens`: Prints the canonical token stream after a successful check.
- `<input.c>`: Names one UTF-8 source file.

You must reject unknown options, missing values, extra input files, and invalid option combinations. Do not invoke preprocessing, type checking, LLVM generation, linking, or runtime execution in this story.

## 4. Standard I/O and Output Formatting Invariants

Write successful command output to `stdout`. Write diagnostics and usage errors to `stderr`. End every output line with a newline character.

For `--check` with valid input, print exactly:
`stricc: ok: <input-path>\n`

For `--emit ast --check`, print one canonical AST line, followed by:
`stricc: ok: <input-path>\n`

For `--emit tokens --check`, print one canonical token line per token, followed by:
`stricc: ok: <input-path>\n`

Do not print success text to `stderr`.

Use this diagnostic format for errors:
`stricc: error: <CODE>: <MESSAGE> at <line>:<column>\n`

Use this usage format for invalid options:
`stricc: usage error: <MESSAGE>\n`

Use these exit statuses:
- `0`: For help, version, and successful front-end checks.
- `1`: For lexical, syntax, unsupported-feature, or symbol diagnostics.
- `2`: For invalid options, missing arguments, unreadable files, or invalid UTF-8.

Keep diagnostic order equal to source order. Report the first fatal error. Use one-based line and column values. Use UTF-8 byte offsets only for internal calculations. Never print UTF-8 byte offsets as columns.

## 5. Explicit Data Representation and Number Formats

The AST must explicitly represent these types:
- `int`: 32-bit signed integer.
- `float`: IEEE-754 binary32 floating point.
- `double`: IEEE-754 binary64 floating point.
- `char`: 8-bit unsigned byte in the front end.
- `_Bool`: Boolean value.
- `void`: Void type.

The AST must explicitly represent these compound types:
- Single-level and multi-level pointers.
- Fixed-size arrays with a decimal, non-negative length.
- Named structures with field declarations.

## 6. Negative Specification Constraints

You must put `#![deny(unsafe_code)]` at the top of every Rust source file. You must not use `unsafe` blocks anywhere in the codebase. You must use safe Rust abstractions for all memory management and tokenization.

## 7. Anti-Stub and Anti-Gaming Invariant

You must not use stubs. You must not use placeholders (for example, `pass`, `...`, `unimplemented!()`). You must not mask shell errors with `|| true`. Do not ignore exit statuses. Do not return unconditional success.

## 8. Chicago-School Testing and In-Memory Test Doubles

You must author and use in-memory test doubles for all file system access in your unit tests and integration tests. You must not read real files from the disk during unit tests. Inject these doubles through traits or generic interfaces. Do not use real network services, wall-clock time, sleeps, or process-global mutable test state.

## 9. Black-Box E2E All-Scenario Invariant

You must run black-box tests against the built binary. You must poll for process readiness when a test needs process readiness. Do not use static sleep delays. You must cover all scenarios in the edge case matrix with black-box tests.

## 10. Deterministic Time and Mock Clock Invariants

This story has no network protocol and no time-based behavior. If you measure compilation time or use any time-based metrics, you must use a deterministic mock clock in your tests.

## 11. Thin Shell Entrypoint Pattern

The `stricc/src/main.rs` file must be a thin wrapper. It must contain fewer than 15 lines of code. It must immediately delegate execution to a callable core function in `stricc/src/driver.rs`.

## 12. Pinned Modular Directory and Required File Paths

You must create or update these files exactly as specified. Do not create monolithic files. Do not use circular imports. Internal modules must import direct module paths. Do not import an internal handler from `lib.rs`, `main.rs`, or another facade module.

- `stricc/src/main.rs`: Thin CLI wrapper.
- `stricc/src/lib.rs`: Public callable driver exports and crate safety attribute.
- `stricc/src/driver.rs`: Argument handling and front-end workflow.
- `stricc/src/error.rs`: Diagnostic codes, locations, formatting, and exit statuses.
- `stricc/src/lexer.rs`: Token types, scanner, and lexer tests.
- `stricc/src/parser.rs`: Recursive descent parser and parser tests.
- `stricc/src/ast.rs`: AST types and canonical rendering.
- `stricc/src/symbol_table.rs`: Scope and symbol operations.
- `stricc/tests/unit/test_lexer.rs`: Lexer unit tests.
- `stricc/tests/unit/test_parser.rs`: Parser unit tests.
- `stricc/tests/unit/test_error.rs`: Diagnostic unit tests.
- `stricc/tests/unit/test_symbol_table.rs`: Symbol table unit tests.
- `stricc/tests/integration/front_end_contract.rs`: Driver integration tests.
- `stricc/tests/black_box/cli_contract.rs`: Executable black-box CLI tests.
- `stricc/tests/fixtures/front_end/valid_minimal.c`: Valid fixture.
- `stricc/tests/fixtures/front_end/valid_supported_subset.c`: Broad valid fixture.
- `stricc/tests/fixtures/front_end/invalid_syntax.c`: Syntax failure fixture.
- `stricc/tests/fixtures/front_end/unsupported_construct.c`: Unsupported feature fixture.
- `stricc/tests/fixtures/front_end/invalid_utf8.bin`: Encoding failure fixture.

## 13. Comprehensive Scenario and Edge Case Matrix

| Scenario | Input Command | Expected Output | Exit Code |
|---|---|---|---|
| Valid C Input | `stricc --check valid.c` | `stricc: ok: valid.c\n` on `stdout` | `0` |
| Invalid Syntax | `stricc --check missing_semi.c` | `stricc: error: E_PARSE_EXPECTED: expected ';' at 10:5\n` on `stderr` | `1` |
| Unsupported Construct | `stricc --check goto.c` | `stricc: error: E_UNSUPPORTED: unsupported construct 'goto' at 5:2\n` on `stderr` | `1` |
| Invalid CLI Option | `stricc --invalid-flag file.c` | `stricc: usage error: unrecognized option '--invalid-flag'\n` on `stderr` | `2` |
| Missing Input File | `stricc --check` | `stricc: usage error: missing input file\n` on `stderr` | `2` |
| Invalid UTF-8 File | `stricc --check invalid_utf8.bin` | `stricc: usage error: invalid UTF-8 encoding\n` on `stderr` | `2` |

## 14. Verification Criteria

You must achieve a 100% pass rate on all unit tests, integration tests, and black-box end-to-end tests. You must have zero test failures.
You must verify exit statuses and exact `stdout` and `stderr` contracts.
You must verify deterministic output across at least three repeated runs.
You must run `cargo fmt --check` with no changes required.
You must run `cargo clippy --workspace --all-targets -- -D warnings` with zero warnings.
You must verify every Rust source file has the no-unsafe policy.

## 15. Machine-Readable Story Contract

```noctifab-contract
{
  "story_id": "US-003",
  "public_contracts": [
    {
      "id": "front-end-cli",
      "interface": "cli",
      "allowed_executables": [
        "target/debug/stricc",
        "target/release/stricc",
        "cargo run -p stricc --"
      ],
      "exit_codes": [0, 1, 2],
      "stdout_contains": [
        "stricc: ok: "
      ],
      "stderr_prefixes": [
        "stricc: error:",
        "stricc: usage error:"
      ]
    }
  ]
}
```
