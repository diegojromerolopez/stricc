# stricc Refined Specification

## 1. Product Scope

Build `stricc`, a deterministic compiler for the supported C subset. Reject unsupported syntax with a diagnostic. Do not claim support for all C programs.

Compile the compiler with Rust and LLVM 18 through `inkwell`. Keep compiler behavior deterministic across runs, optimization levels, and supported targets.

## 2. Rust Safety Rule

Place `#![deny(unsafe_code)]` in every Rust crate root. Permit zero `unsafe` blocks, unsafe functions, unsafe traits, raw-pointer dereferences, and unsafe attributes.

Use safe Rust collections, ownership, borrowing, locks, and standard file APIs.

Do not implement a raw host allocator or raw-pointer dereference in the runtime while this rule remains active. Rust safe code cannot provide that operation. Use a safe runtime memory model based on Rust-owned allocation records and integer handles for the initial release.

Mark direct ABI-compatible raw-pointer allocation as out of scope. Reject source operations that require an untracked external raw pointer. Add a future design review before enabling such ABI support.

## 3. Supported C Subset

Support these scalar types: `int` as signed 32-bit, `float` as IEEE-754 binary32, `double` as IEEE-754 binary64, `char` as 8-bit, `bool` as one Boolean value, and `void`.

Support pointers, fixed-size arrays, named structures, `const`, `nullptr`, `auto`, and `constexpr`.

Support declarations, assignments, function definitions, function calls, `if`, `else`, `while`, `for`, `return`, `break`, and `continue`.

Support arithmetic, bitwise operations, comparisons, array access, structure access, address-of, dereference, and calls.

Reject unions, variadic functions, flexible arrays, variable-length arrays, unsupported casts, inline assembly, and untracked foreign pointers.

## 4. Compiler Pipeline

Implement this pipeline:

1. Parse command-line options.
2. Read source files.
3. Tokenize source text.
4. Parse the supported grammar.
5. Build the typed AST.
6. Run semantic checks and value-range analysis.
7. Generate LLVM IR.
8. Link generated code with the safe runtime where required.
9. Report diagnostics and process status.

Keep each stage testable through public Rust functions. Keep `main.rs` as a thin wrapper that delegates to the driver.

## 5. Deterministic Semantics

Trap signed integer overflow. Trap division by zero. Trap `INT_MIN / -1`. Mask integer shift counts with the operand width minus one. Apply IEEE-754 operations for floating-point values.

Define evaluation order for every supported expression. Reject expressions whose required order cannot be represented deterministically.

Initialize every scalar and pointer local before first use. Initialize integers and floating values to zero. Initialize pointers to `nullptr`.

Reject a returned address that refers to a local stack object. Reject control flow that reaches the end of a non-void function.

Represent invalid memory access as a compile-time diagnostic when analysis proves the fault. Otherwise emit a deterministic runtime trap.

## 6. Memory Safety

Track compiler-created objects with allocation records. Each record contains an object identifier, byte size, lifetime state, and source location.

Check null access, array bounds, structure field bounds, use after release, and repeated release. Emit a deterministic trap with source file, line, column, and a stable error category.

Use the safe runtime handle model for dynamic allocation. Do not expose raw allocation addresses through the public runtime contract.

Use value-range propagation to remove a bounds check only after it proves the complete access range. Preserve the check when proof is incomplete.

## 7. Diagnostics and Exit Codes

Write normal compiler output to stdout. Write diagnostics to stderr.

Use the prefix `error:` for compilation errors and `warning:` for warnings. Use exit code `0` for successful compilation, `1` for compilation failure, and `2` for invalid command-line options.

Runtime safety traps must write `stricc_abort: <category>: <file>:<line>:<column>` to stderr and exit with code `134`.

Diagnostics must include source location, category, and a stable explanatory message.

## 8. Command-Line Contract

Provide this executable: `./target/debug/stricc`.

Support `stricc --help`, `stricc --version`, `stricc -o <output> <input.c>`, and `stricc --emit-llvm <input.c>`.

Reject missing input, duplicate output options, unknown options, unreadable files, and invalid output paths.

`--help` and `--version` exit with code `0`. Invalid options exit with code `2`. Compilation errors exit with code `1`.

## 9. Required Repository Layout

Use these modular files:

- `stricc/src/main.rs`
- `stricc/src/driver.rs`
- `stricc/src/cli.rs`
- `stricc/src/lexer.rs`
- `stricc/src/parser.rs`
- `stricc/src/ast.rs`
- `stricc/src/symbol_table.rs`
- `stricc/src/typechecker.rs`
- `stricc/src/vrp.rs`
- `stricc/src/codegen.rs`
- `stricc/src/diagnostics.rs`
- `runtime/src/lib.rs`

Place unit tests beside the matching module or in a matching `stricc/tests/unit/` directory. Place black-box C programs in `stricc/tests/defined/` and `stricc/tests/safety/`.

## 10. Verification

Run `cargo test --workspace`. Run `cargo fmt --check`. Run `cargo clippy --workspace -- -D warnings`.

Run `make test-gcc` and `make test-llvm` when the required external suites exist. Report unavailable suites as infrastructure failures, not successful tests.

Use Chicago-school tests with real compiler objects and in-memory test doubles for external boundaries. Use black-box tests for every CLI option and every safety rule. Give every process and network read a maximum timeout of two seconds.

Use deterministic clocks for any future time-based feature. Do not use static sleep delays for readiness checks.

Do not use `docker-compose`. If container tests become necessary, use `docker compose -f docker-compose.e2e.yml up --build --exit-code-from <service>`.

Do not use stubs, placeholders, empty tests, tautological assertions, or shell error masking.
