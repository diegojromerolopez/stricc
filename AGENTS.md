# AI Agent Instructions (`stricc`)

This guide is designed to orient AI agents and LLMs working on the `stricc` compiler codebase. Please follow these guidelines, architecture structures, and instructions.

---

## 📖 Essential Documentation
Before proposing or making modifications, review the core design specs:
* **[PLAN.md](file:///Users/diegoj/repos/stricc/PLAN.md)**: The canonical compiler blueprint, outlining how `stricc` mitigates every C undefined behavior (UB), and phase goals.
* **[README.md](file:///Users/diegoj/repos/stricc/README.md)**: Main developer setup and build instructions.
* **[TEST.md](file:///Users/diegoj/repos/stricc/TEST.md)**: Complete test strategy, execution guides, and coverage metrics.

---

## 🛠️ Linter & Formatting Guardrails
We maintain strict code quality standards. **Both linters (cargo fmt and cargo clippy) must pass without warnings or errors.** Ensure you run these commands before finalizing any changes:
* **Code Formatting**:
  ```bash
  cargo fmt --all
  ```
* **Linter Warnings (Deny All)**:
  ```bash
  cargo clippy --workspace --all-targets -- -D warnings
  ```

---

## 🧪 Testing Suites & Validation
**All tests must pass successfully after any change in the code.** Verify changes using the following three categories of tests:

### 1. Unit Tests
Located in `stricc/tests/unit/`:
* `cargo test --workspace` or specific target tests:
  ```bash
  cargo test --test test_lexer --verbose
  cargo test --test test_parser --verbose
  cargo test --test test_typechecker --verbose
  cargo test --test test_codegen --verbose
  cargo test --test test_error --verbose
  ```

### 2. Integration & Compliance Suites
Located in `stricc/tests/`:
* **GCC C Torture Execution Suite**: Checks execution logic against GCC conformance standards.
  ```bash
  python3 stricc/tests/gcc_torture_runner.py
  ```
* **LLVM SingleSource Unit Tests**: Checks correctness against LLVM compiler tests.
  ```bash
  python3 stricc/tests/llvm_test_suite_runner.py
  ```

### 3. Spatial & Temporal Safety Tests
Ensure safety traps are correctly emitted:
* **`stricc/tests/defined/`**: C programs representing UB scenarios that `stricc` compiles with *predictable/defined behavior* (e.g. wrapping signed overflow, defined bitcasts).
* **`stricc/tests/safety/`**: C programs that MUST trigger runtime safety checks and cause a clean runtime abort (e.g., division by zero, null pointer dereference, use-after-free).

### 4. Sandbox Validation via Docker
If the local machine lacks LLVM 18 or Rust, changes can be validated inside the Docker sandbox:
```bash
# Build the test image
docker build -f Dockerfile.run -t stricc-sandbox .

# Compile and execute a test file inside the sandbox
docker run --rm -v "$(pwd)":/src stricc-sandbox stricc -o test input.c
docker run --rm -v "$(pwd)":/src stricc-sandbox ./test
```

---

## 🧱 Software Engineering Guardrails

When modifying the compiler or runtime library, adhere to these architectural standards:

1. **SOLID Principles**:
   * *Single Responsibility*: Do not mix AST representation, parsing logic, semantic typing, and codegen in the same module.
   * *Interface Segregation / Open-Closed*: Allow easy addition of new AST node forms or semantic analyzer check passes without modifying the core compiler pipeline.

2. **Domain-Driven Design (DDD)**:
   * Keep clean boundaries between the core domain contexts:
     * **Parser Domain**: Syntactic processing of source tokens to AST.
     * **Type Checker/Semantic Domain**: Variable/scope analysis, escape analysis, and type validation.
     * **LLVM Codegen Domain**: Transmuting annotated AST to LLVM IR.
     * **Runtime Domain**: Lower-level shadow table checks and symbolicated diagnostic aborts.

3. **Dependency Injection (DI)**:
   * *Emphasize Injectable Dependencies*: Ensure components like the `TypeChecker`, `Parser`, and `Codegen` receive their resources (such as `SymbolTable`, diagnostic handler, or `LLVMContext`) via constructor injection rather than using global states or hard-coded static dependencies.
   * Make structs modular and mockable to facilitate precise unit testing of components in isolation.

---

## 📂 Project Directory Structure

```
stricc/
├── .github/workflows/          # GitHub Actions pipelines (CI, unit tests, builds, release)
├── runtime/                    # Runtime support library (libstricc_rt)
│   ├── Cargo.toml
│   └── src/
│       └── lib.rs              # Shadow metadata table, signal handler, stdlib shims
├── stricc/                     # Compiler frontend & middle-end
│   ├── Cargo.toml
│   ├── src/
│   │   ├── lib.rs              # Module declarations
│   │   ├── main.rs             # Driver executable entry point
│   │   ├── ast.rs              # Abstract Syntax Tree (AST) definitions
│   │   ├── lexer.rs            # Lexer/Tokenizer scanning C source code
│   │   ├── parser.rs           # Recursive descent parser with recovery
│   │   ├── symbol_table.rs     # Scoped symbol tables
│   │   ├── typechecker.rs      # Semantic analysis, VRP, escape analysis
│   │   ├── codegen.rs          # LLVM IR Emitter using inkwell
│   │   ├── error.rs            # Compiler diagnostic error reporting
│   │   └── driver.rs           # Compilation workflow controller (preprocess -> link)
│   └── tests/
│       ├── defined/            # Safe/defined UB execution tests
│       ├── safety/             # Runtime abort/safety violation tests
│       ├── unit/               # Rust unit tests
│       ├── gcc_torture_runner.py
│       └── llvm_test_suite_runner.py
├── Cargo.toml                  # Workspace root configuration
├── Dockerfile.run              # Sandbox container for running/testing the compiler
├── Makefile                    # Build scripts for macOS and static Linux builds
├── PLAN.md                     # Compiler blueprint/UB specs
├── README.md                   # Setup guide
└── TEST.md                     # Test strategy documentation
```

---

## 🚀 GitHub Workflows

The repository leverages five automated workflows:
1. **Lint (`.github/workflows/ci.yml`)**: Verifies code formatting via `rustfmt` and checks for clippy lints. Runs integration suites (GCC Torture, LLVM SingleSource).
2. **Unit Tests (`.github/workflows/test_unit.yml`)**: Runs Rust unit tests and generates code coverage metrics with `cargo-llvm-cov`.
3. **External Builds Stress Test (`.github/workflows/test_builds.yml`)**: Stress-tests compilation of external, real-world C projects (SQLite, Redis, TinyCC) using the freshly compiled `stricc` binary.
4. **Release (`.github/workflows/release.yml`)**: Packages static Linux and native macOS binaries to Github Releases.
5. **Docker Build & Test (`.github/workflows/docker_test.yml`)**: Verifies building the `Dockerfile.run` container and compiles/executes a test program inside the runner image to ensure end-to-end compiler correctness.
