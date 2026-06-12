# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.1.1] - 2026-06-12

### Added
- GitHub Actions workflow (`.github/workflows/test_build_apps.yml`) to compile and link classic C-programs/applications (SQLite CLI shell, Doom generic targets, Lua interpreter, and minilisp) using the `stricc` compiler.
- Makefile targets (`test-build-apps`, `test-build-sqlite`, `test-build-doom`, `test-build-lua`, and `test-build-lisp`) to clone, build, and test compiling the classic C applications locally.
- Compiler command-line interface argument filtering in `stricc/src/main.rs` to automatically strip unsupported GCC/Clang options (such as `-g`, `-Wall`, `-Wextra`, etc.) before parsing, enabling drop-in compatibility with standard build systems.
- Build script (`stricc/build.rs`) to compile the `runtime` static library dynamically matching the target architecture during compiler build.
- Compile-time embedding of the runtime library (`libstricc_rt.a`) static archive bytes directly inside the `stricc` binary.
- Helper function `get_runtime_lib_tempfile()` to write the embedded static library to a temporary file for linking.
- Sphinx/Read the Docs and workspace markdown documentation (cli_usage, developer guide, README, and TEST.md) covering the CLI option pre-filtering and the real-world application test build suite.

### Changed
- Refactored compiler driver linking orchestration (`driver.rs` and `main.rs`) to use the embedded runtime library tempfile, making the resolution of a standalone `libstricc_rt.a` file on the host filesystem obsolete.
- Updated `Makefile` to only output the standalone compiler executable.
- Simplified `Dockerfile.build.linux` and `Dockerfile.build.macos` to only output the compiler executable.

### Fixed
- Parser panic/out-of-bounds crash in `stricc/src/parser.rs` by adding bounds safety checks to `current_token()`, `advance()`, and `is_at_end()`, and ensuring a fallback EOF token is always present even in case of lexer errors.
- Integration test runner race condition in `stricc/tests/runner.rs` by wrapping the compiler build in a `std::sync::Once` block to prevent parallel test threads from corrupting the compiler binary.

### Removed
- standalone `libstricc_rt.a` from compiled distributions, run image (`Dockerfile.run`), and GitHub Actions release assets workflow.

## [0.1.0] - 2026-06-12

### Added
- Initial release of the `stricc` compiler frontend (written in Rust) and the runtime support library `libstricc_rt` (written in Rust).
- Support for compiling C files into safe executables with runtime safety check instrumentation.
- Shadow metadata table tracking for pointers.
- Safe runtime shims/interceptors for `malloc`, `free`, `realloc`, `calloc`, and standard library string-processing functions.
- GCC C Torture Execution Suite and LLVM SingleSource testing integration.
