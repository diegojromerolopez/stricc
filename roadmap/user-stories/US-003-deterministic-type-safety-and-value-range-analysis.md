# US-002: Deterministic Type Safety and Value Range Analysis

## Story

As a compiler user, I want deterministic semantic checks and safety decisions, so every accepted program has defined behavior.

## Depends On

- US-002

## Complexity

34 CU

## Domain Capability

Semantic Analysis, Lifetime Rules, and Value Range Propagation

## Scope

Implement type checking for the supported C subset. Track declaration state, object lifetime, and value ranges. Reject unsafe operations when static proof fails. Preserve a runtime check for unsafe access when later code generation requires one.

Keep this story inside semantic analysis. Do not implement LLVM lowering or runtime allocation in this story.

## Observable Entry Point

Use the public compiler command from the repository root.

- Build: `cargo build --workspace --locked`
- Check source: `target/debug/stricc --check <input.c>`
- Emit diagnostics: `target/debug/stricc --check <input.c> 2> <diagnostics.txt>`
- Run unit tests: `cargo test --workspace --locked`
- Run lint checks: `cargo clippy --workspace --all-targets --all-features -- -D warnings`
- Check format: `cargo fmt --all -- --check`

The `--check` command parses and type-checks one source file. It does not emit LLVM or link a program.

Accept one input path. Reject a missing path, a directory, an unreadable file, or extra positional arguments. Return status `0` only after all checks pass. Return a non-zero status for every diagnostic or command error.

## Command Contract

Use this output contract for `--check`.

- Write no output to stdout when checking succeeds.
- Write no output to stdout when checking fails.
- Write one diagnostic per line to stderr.
- Start every compiler diagnostic with `error:`.
- Use this form: `error: <category>: <message> at <path>:<line>:<column>`.
- Use stable category names: `type`, `init`, `lifetime`, `arithmetic`, `shift`, `range`, `control`, and `cli`.
- Use one-based line and column numbers.
- Preserve the input path text in each diagnostic.
- Return `0` for valid input.
- Return `1` for semantic or syntax failure.
- Return `2` for CLI, file, or internal command failure.

Do not promise a diagnostic order that depends on hash-map iteration. Visit declarations and expressions in source order. Report the first error in each source-order path. Permit multiple independent diagnostics only when the order remains source order.

## Semantic Model

### Primitive Types

Use these fixed representations.

| Type | Representation | Range or rule |
|---|---|---|
| `int` | signed 32-bit integer | `-2147483648` through `2147483647` |
| `char` | unsigned 8-bit byte | `0` through `255` |
| `bool` | Boolean | `false` or `true` |
| `float` | IEEE-754 binary32 | Preserve finite, infinity, and NaN values |
| `double` | IEEE-754 binary64 | Preserve finite, infinity, and NaN values |
| `void` | no value | Use only where the grammar permits it |
| pointer | typed address value | `nullptr` or a typed object address |
| fixed array | element type plus constant length | Length is a non-negative `usize` value |
| struct | named field set | Field names are unique within the struct |

Do not use host integer width for semantic results. Use explicit 32-bit arithmetic for `int`. Use explicit 8-bit storage for `char`.

### Type Rules

Implement these rules.

1. Resolve every variable, function, struct, and field name before use.
2. Reject an unknown name with category `type`.
3. Require assignment compatibility between the destination and source type.
4. Permit `nullptr` for pointer destinations.
5. Require integer operands for `%`, bitwise operators, and shifts.
6. Require compatible numeric operands for arithmetic and comparisons.
7. Require a Boolean result for conditions after permitted scalar conversion.
8. Require an lvalue for assignment, address-of, and writable access.
9. Reject writes through `const` objects.
10. Require a complete return value for every non-void return statement.
11. Reject a `return` expression in a `void` function.
12. Reject a call with an unknown function, wrong arity, or incompatible argument type.
13. Require an array or pointer base for indexing.
14. Require a non-negative integer index expression when a constant value proves negativity.
15. Require a struct base for `.` and a pointer-to-struct base for `->`.
16. Reject access to an unknown struct field.
17. Reject dereference of a value with a non-pointer type.
18. Reject conversion from `void` to a value type.
19. Reject unsupported implicit pointer conversions.
20. Keep type decisions independent of optimization level and host target.

### Initialization and Lifetime Rules

Track a state for each local object: `Uninitialized`, `Initialized`, `Moved`, or `Invalid`. Track an allocation state for managed objects: `Live`, `Freed`, or `Unknown`.

Apply these rules.

- Initialize scalar and pointer locals with zero and `nullptr` when the language rule permits automatic initialization.
- Reject a read from an object that has state `Uninitialized` when no automatic initialization rule applies.
- Mark an object `Initialized` after a valid assignment or initializer.
- Reject a write to a `const` object after initialization.
- Reject an address that escapes its local stack lifetime.
- Reject returning the address of a local object.
- Reject use of an object after its lifetime ends.
- Reject a double release when static analysis proves the object is already freed.
- Preserve a runtime lifetime check when the state is `Unknown`.
- Reject control flow that reaches the end of a non-void function.
- Merge branch states conservatively. Use `Unknown` when branches disagree.
- Merge loop states conservatively until a fixed point or a bounded iteration limit.

### Deterministic Arithmetic

Represent signed `int` operations with checked 32-bit semantics.

- Prove addition, subtraction, and multiplication inside the `int` range when possible.
- Emit an arithmetic safety requirement when proof fails.
- Mark signed division by zero as an arithmetic error when the divisor is constant zero.
- Mark `INT_MIN / -1` as an arithmetic error when both values are known.
- Preserve a runtime arithmetic guard when either divisor value is unknown.
- Treat floating-point operations as IEEE-754 operations.
- Do not convert a floating-point exception into a host-dependent compiler result.
- Do not use wrapping signed arithmetic to hide a failed proof.

The semantic result must identify one of these states: `ProvenSafe`, `CompileError`, or `NeedsRuntimeCheck`.

### Shift Rules

Use a 32-bit mask for `int` shift counts.

- Mask every `int` shift count with `count & 31`.
- Fold a constant count with the same mask.
- For example, `35` becomes `3`.
- Reject a non-integer shift count.
- Reject a non-integer shift value.
- Preserve the masked count in the semantic result.
- Do not depend on the host language shift behavior.

### Value Range Propagation

Represent an integer range as inclusive bounds with an optional `Unknown` state. Represent Boolean values as `{false}`, `{true}`, or `{false,true}`. Preserve a type-specific range for `char` and `int`.

Propagate ranges through literals, assignments, arithmetic, comparisons, conditional branches, and loop conditions.

Implement these proofs.

- Prove `0 <= i < N` from loop initialization and a matching loop condition.
- Prove an array access when the index range is inside `[0, length - 1]`.
- Prove a pointer offset when the full byte range stays inside the known object.
- Prove division safety when the divisor range excludes zero and excludes the `INT_MIN / -1` pair.
- Prove a shift count after applying the mask.
- Preserve `NeedsRuntimeCheck` when a proof is incomplete.
- Never remove a check because an unknown range appears likely to be safe.

For `int arr[10]` and `for (int i = 0; i < 10; i++)`, prove `i` in `[0,9]`. Mark `arr[i]` as `ProvenSafe`. For `arr[i]` with an unknown `i`, mark the access as `NeedsRuntimeCheck`.

## Required Repository Paths

Create or update only the following paths for this story.

- `stricc/src/typechecker.rs`: semantic checks, state tracking, arithmetic rules, and VRP.
- `stricc/src/symbol_table.rs`: typed symbols, scopes, declarations, and lifetime metadata.
- `stricc/src/ast.rs`: semantic annotations and source spans, if the existing AST lacks them.
- `stricc/src/error.rs`: stable diagnostic categories, messages, locations, and exit mapping.
- `stricc/src/driver.rs`: invoke semantic analysis for `--check` and map results to the CLI contract.
- `stricc/src/main.rs`: keep the existing thin CLI wrapper and forward command work to the driver.
- `stricc/tests/unit/test_typechecker.rs`: unit tests for types, initialization, lifetime, arithmetic, shifts, and ranges.
- `stricc/tests/unit/test_error.rs`: diagnostic text, location, category, and exit-code tests.
- `stricc/tests/integration/type_safety.rs`: in-process compiler pipeline tests.
- `stricc/tests/black_box/type_safety_cli.rs`: executable CLI tests for all command contracts.
- `stricc/tests/fixtures/us003/`: valid and invalid C fixtures grouped by semantic rule.

Keep internal modules independent. Do not import an internal handler or analyzer through `lib.rs`, `main.rs`, or another facade module. Import the smallest required module directly.

## Test Doubles and Test Design

Use Chicago-school tests. Test observable state and behavior through public component calls.

Create these in-memory test doubles under the test modules.

- `InMemorySource`: stores source text and a stable virtual path.
- `InMemoryDiagnostics`: captures stderr records without filesystem output.
- `InMemorySymbolStore`: provides deterministic symbol and scope storage.
- `InMemoryRangeFacts`: records range facts and proof decisions.
- `InMemoryClock`: provide it only if a legacy interface requests time; semantic analysis must not use time.

Do not call the real filesystem in unit tests. Use the in-memory doubles for unit and integration tests. Use temporary files only in black-box CLI tests.

## Scenario Matrix

| ID | Input or condition | Expected result | Status |
|---|---|---|---|
| S01 | Valid typed declaration and assignment | No diagnostic; exit `0` | `ProvenSafe` |
| S02 | Unknown variable | `error: type:` on stderr; exit `1` | Reject |
| S03 | Assignment between incompatible types | `error: type:`; exit `1` | Reject |
| S04 | `nullptr` assigned to a pointer | No diagnostic; exit `0` | `ProvenSafe` |
| S05 | Dereference of an integer | `error: type:`; exit `1` | Reject |
| S06 | Write through `const` | `error: type:`; exit `1` | Reject |
| S07 | Read of an uninitialized object | `error: init:`; exit `1` | Reject or apply the stated zero rule |
| S08 | Address of a local returned | `error: lifetime:`; exit `1` | Reject |
| S09 | Non-void function reaches its end | `error: control:`; exit `1` | Reject |
| S10 | Constant `INT_MAX + 1` | `error: arithmetic:`; exit `1` | Reject |
| S11 | Unknown-value addition | No compile error; retain runtime requirement | `NeedsRuntimeCheck` |
| S12 | Division by constant zero | `error: arithmetic:`; exit `1` | Reject |
| S13 | `INT_MIN / -1` | `error: arithmetic:`; exit `1` | Reject |
| S14 | Unknown divisor range excluding no values | Retain division guard | `NeedsRuntimeCheck` |
| S15 | Constant shift count `35` | Mask to `3`; no diagnostic | `ProvenSafe` |
| S16 | Non-integer shift count | `error: shift:`; exit `1` | Reject |
| S17 | `arr[i]` with `i` in `[0,9]` for length `10` | Mark access safe | `ProvenSafe` |
| S18 | `arr[i]` with unknown `i` | Retain bounds check | `NeedsRuntimeCheck` |
| S19 | Constant index `10` for length `10` | `error: range:`; exit `1` | Reject |
| S20 | Unknown field or wrong arrow form | `error: type:`; exit `1` | Reject |
| S21 | Empty source file | Apply parser contract; return deterministic result | Defined |
| S22 | Missing input path | `error: cli:`; exit `2` | Reject |
| S23 | Directory input | `error: cli:`; exit `2` | Reject |
| S24 | Extra CLI argument | `error: cli:`; exit `2` | Reject |
| S25 | Very large integer literal | `error: type:` or `error: range:`; exit `1` | Reject |
| S26 | Nested branch and loop state merge | Use conservative `Unknown` facts | Defined |
| S27 | Repeated compilation of the same input | Identical diagnostics and decisions | Deterministic |
| S28 | Different host optimization levels | Identical semantic result | Deterministic |

Cover every row with at least one automated test. Add boundary tests for `INT_MIN`, `INT_MAX`, zero, `-1`, shift counts `0`, `31`, `32`, and `35`, and array indexes `-1`, `0`, `length-1`, and `length`.

## Anti-Stub and Negative Constraints

Do not add `pass`, `...`, `TODO` as an implementation, `NotImplementedError`, empty success branches, placeholder values, or disabled assertions. Do not mask failures with `|| true`, ignored exit codes, or unconditional test skips.

Do not add Rust `unsafe` code. Preserve `#![deny(unsafe_code)]` in every Rust crate root. Do not use raw-pointer dereferences in compiler code. Do not claim runtime allocation, LLVM emission, ABI support, or backtrace support in this story.

Do not use host-dependent integer widths, locale-dependent diagnostics, random ordering, wall-clock time, or network access. Do not change parser behavior outside the US-003 semantic contract without a characterization test.

## Definition of Done

1. Implement all type, initialization, lifetime, arithmetic, shift, and range rules in this story.
2. Return stable diagnostics with the required category, location, text prefix, and status.
3. Produce `ProvenSafe`, `CompileError`, or `NeedsRuntimeCheck` for every checked unsafe operation.
4. Preserve runtime-check requirements for every unproven access.
5. Prove the bounded loop example without a bounds check requirement.
6. Keep semantic results identical across repeated runs and host optimization levels.
7. Add the required in-memory test doubles and use them in unit and integration tests.
8. Add black-box tests for every scenario in the matrix.
9. Keep all required module paths and avoid facade imports from internal modules.
10. Pass `cargo fmt --all -- --check` with zero errors.
11. Pass `cargo clippy --workspace --all-targets --all-features -- -D warnings` with zero warnings.
12. Pass `cargo test --workspace --locked` with zero failures.
13. Pass all US-003 black-box tests with socket readiness polling if a future test harness uses a socket; never use static sleep delays.
14. Verify that no test or command masks a failure.
15. Confirm a 100% pass rate for unit, integration, and black-box suites.

## Validation Commands

Run these commands from the repository root.

1. `cargo fmt --all -- --check`
2. `cargo clippy --workspace --all-targets --all-features -- -D warnings`
3. `cargo test --workspace --locked`
4. `cargo build --workspace --locked`
5. `target/debug/stricc --check stricc/tests/fixtures/us003/valid_vrp.c`
6. `target/debug/stricc --check stricc/tests/fixtures/us003/invalid_div_zero.c`
7. `target/debug/stricc --check stricc/tests/fixtures/us003/invalid_lifetime.c`
8. `target/debug/stricc --check stricc/tests/fixtures/us003/invalid_bounds.c`

Compare stdout, stderr, and exit status with the contract. Run each deterministic case twice. Compare the complete results byte for byte.

```noctifab-contract
{
  "story_id": "US-002",
  "public_contracts": [
    {
      "id": "US003-CHECK-CLI",
      "interface": "target/debug/stricc --check <input.c>",
      "allowed_executables": ["cargo", "target/debug/stricc"],
      "exit_codes": {"success": 0, "semantic_error": 1, "command_error": 2},
      "stdout_contains": [],
      "stderr_prefixes": ["error: type:", "error: init:", "error: lifetime:", "error: arithmetic:", "error: shift:", "error: range:", "error: control:", "error: cli:"]
    }
  ]
}
```
