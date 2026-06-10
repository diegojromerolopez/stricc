# Developer Guide & Contributing

If you are looking to work on the `stricc` compiler itself, this guide will help you set up your environment, understand the codebase layout, and run validation suites.

---

## 1. Developer Prerequisites

`stricc` relies on the `inkwell` crate to bind to the LLVM 18 C++ APIs. In addition to installing LLVM 18, your compiler needs to be able to locate the `llvm-config` binary during the build process.

### macOS (Homebrew)
Install LLVM 18:
```bash
brew install llvm@18
```
Before running `cargo build` or `cargo test`, set the following environment variables so `inkwell` can find the LLVM library:
```bash
export LLVM_SYS_180_PREFIX="/opt/homebrew/opt/llvm@18"
export PATH="/opt/homebrew/opt/llvm@18/bin:$PATH"
```

### Ubuntu/Debian
Ensure LLVM 18 is installed and `llvm-config-18` is in your `PATH` or symlinked as `llvm-config`.
```bash
sudo apt-get install llvm-18 llvm-18-dev clang-18
```

---

## 2. Codebase & Directory Layout

The workspace is divided into two primary crates: the compiler driver and the runtime support library.

- **`stricc/`**: The compiler implementation.
  - `src/lexer.rs`: Tokenizes the preprocessed C source code.
  - `src/parser.rs`: Hand-written recursive descent parser that parses tokens into the Abstract Syntax Tree (AST).
  - `src/ast.rs`: Defines the AST nodes.
  - `src/typechecker.rs`: Performs type validation, Value Range Propagation (VRP) to optimize bounds checks, escape analysis, and constant folding.
  - `src/codegen.rs`: Translates the AST to LLVM IR using `inkwell` and injects safety checks.
  - `src/driver.rs`: Coordinates the compiler stages and manages host preprocessor delegation.
  - `src/main.rs`: Entrypoint for CLI parsing and flag emulation.
- **`runtime/`**: The runtime support library (`libstricc_rt.a`).
  - `src/lib.rs`: Implements the memory allocation wrappers (`malloc`/`free`), shadow metadata table operations, stack overflow/signal handlers, and symbolicated DWARF backtrace reporting.

Refer to the detailed specifications and architectural blueprints in [PLAN.md](../PLAN.md) and developer safety constraints in [AGENTS.md](../AGENTS.md).

---

## 3. Local Development & Advanced Testing

The root directory contains a `Makefile` that simplifies building and running the extended test suites:

- **Build the workspace**:
  ```bash
  make build
  ```
- **Run all unit/integration tests**:
  ```bash
  make test
  ```
- **Run the GCC C Torture Suite** (compares execution logic against GCC for ~1,500 test cases):
  ```bash
  make test-gcc
  ```
- **Run the LLVM Test Suite**:
  ```bash
  make test-llvm
  ```

Refer to [TEST.md](../TEST.md) for a complete breakdown of the safety matrix and conformance suites.

---

## 4. Code Formatting & Linting

We enforce strict quality control. Before proposing code changes, format and lint the workspace:

- **Code Formatting**:
  ```bash
  cargo fmt --all
  ```
- **Linter warnings (Deny All)**:
  ```bash
  cargo clippy --workspace --all-targets -- -D warnings
  ```

---

## Future Improvements

To transition `stricc` from a prototype to a production-grade compiler, the following roadmap of future improvements is planned:

- **Documentation of Source Code**: Enhance inline code comments, expand Rustdoc documentation for internal compiler API interfaces (lexer, parser, typechecker, and codegen modules), and publish comprehensive design details.
- **Limitation of Source Code File Size**: Implement strict compiler guards or compiler limits on source file sizes, token counts, and recursion depth to prevent denial-of-service (DoS) style stack exhaustion during parsing and analysis phases.
- **Ensuring Idiomatic Rust**: Refactor legacy areas of the compiler backend to use idiomatic Rust patterns (e.g., proper error propagation with `Result`/`Option`, zero-copy parsing where feasible, and avoiding unnecessary clones/unwraps).
- **Performance Metrics & Benchmarking**: Integrate automated benchmarking suites (e.g., using `criterion`) to track compile-time performance, memory consumption of the compiler, and runtime overhead of safety-instrumented executables.
- **More Tests**: Expand the test suites to include more edge cases, deeper integration tests for complex pointer aliasing, and larger real-world C codebases to improve coverage and reliability.
- **Alternative Backends & LLVM Removal**: Investigate replacing the LLVM/Inkwell backend with a custom lightweight backend or code generator to reduce compilation dependencies, build times, and runtime binary footprint.
