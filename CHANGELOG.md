# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.1.1] - 2026-06-12

### Added
- Build script (`stricc/build.rs`) to compile the `runtime` static library dynamically matching the target architecture during compiler build.
- Compile-time embedding of the runtime library (`libstricc_rt.a`) static archive bytes directly inside the `stricc` binary.
- Helper function `get_runtime_lib_tempfile()` to write the embedded static library to a temporary file for linking.

### Changed
- Refactored compiler driver linking orchestration (`driver.rs` and `main.rs`) to use the embedded runtime library tempfile, making the resolution of a standalone `libstricc_rt.a` file on the host filesystem obsolete.
- Updated `Makefile` to only output the standalone compiler executable.
- Simplified `Dockerfile.build.linux` and `Dockerfile.build.macos` to only output the compiler executable.

### Removed
- standalone `libstricc_rt.a` from compiled distributions, run image (`Dockerfile.run`), and GitHub Actions release assets workflow.

## [0.1.0] - 2026-06-12

### Added
- Initial release of the `stricc` compiler frontend (written in Rust) and the runtime support library `libstricc_rt` (written in Rust).
- Support for compiling C files into safe executables with runtime safety check instrumentation.
- Shadow metadata table tracking for pointers.
- Safe runtime shims/interceptors for `malloc`, `free`, `realloc`, `calloc`, and standard library string-processing functions.
- GCC C Torture Execution Suite and LLVM SingleSource testing integration.
