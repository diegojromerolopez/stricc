# CLI Usage & Integration Guide

This guide describes how to install, build, and use the `stricc` compiler to build safe C programs. It covers command-line interface flags, compilation examples, and integration with common build systems like `Make` and `CMake`.

## Prerequisites

`stricc` relies on **LLVM 18** for code generation and optimizations. You must install LLVM 18 on your host system before building:

- **macOS**:
  ```bash
  brew install llvm@18
  ```
- **Ubuntu/Debian**:
  Follow the instructions on [apt.llvm.org](https://apt.llvm.org/) to install the repository and run:
  ```bash
  sudo apt-get install llvm-18 llvm-18-dev clang-18
  ```

---

## Building the Compiler

To build the `stricc` compiler driver and its safe runtime support library (`libstricc_rt.a`), run Cargo from the root of the repository:

```bash
cargo build --release
```

This compiles:
1. The compiler driver binary at `./target/release/stricc`
2. The runtime helper library at `./target/release/libstricc_rt.a` (used for symbolicated backtraces and shadow memory management).

---

## Compiling C Code

The `stricc` compiler driver is designed to mimic `gcc`. Here are typical usage examples:

### Compile to an Executable
Compile a C file directly to a safe, runnable binary. `stricc` automatically links against its runtime library (`libstricc_rt.a`):
```bash
./target/release/stricc -o app main.c
```

### Compile to an Object File
To compile a source file to a machine code object file without linking:
```bash
./target/release/stricc -c -o helper.o helper.c
```

### High-Optimization with Headers
Compile a file with optimization level 3 and add include search directories:
```bash
./target/release/stricc -O3 -I./include -o app main.c
```

---

## Supported Command-Line Options

The CLI parser supports the following common GCC options:

| Option | Description | `stricc` Behavior |
| :--- | :--- | :--- |
| `-o <file>` | Place output binary/file in `<file>` | Outputs compiled binary or target file to the designated path. |
| `-c` | Compile and assemble, but do not link | Generates standard machine code object file (`.o`). |
| `-S` | Compile only; do not assemble or link | Generates text-based LLVM-translated assembly file (`.s`). |
| `-emit-llvm` | Output LLVM IR | Generates human-readable LLVM IR assembly file (`.ll`). |
| `-E` | Preprocess only | Delegates to host preprocessor and prints preprocessed source to stdout. |
| `-O0`, `-O1`, `-O2`, `-O3` | Optimization levels | Translates directly to LLVM optimization and vectorization pipeline levels. |
| `-I <dir>` | Add directory to include search path | Appends directory to host preprocessor include directories. |
| `-D <macro>[=val]` | Define preprocessor macro | Passes preprocessor macro definitions downstream. |
| `-Wall`, `-Wextra` | Enable compilation warnings | *Ignored/Filtered* (stripped automatically for build system compatibility). |
| `-std=<val>` | Specify language standard | *Ignored/Filtered* (stripped automatically for build system compatibility). |

---

## CLI Option Pre-Filtering for Compatibility

To allow easy integration with existing codebases, build systems (such as `Make` and `CMake`), and build scripts, `stricc` implements a pre-filtering pass on CLI arguments. Before parsing arguments via `clap`, any unsupported compiler options starting with `-` are silently stripped/ignored instead of failing compilation.

Common compiler flags that are automatically ignored include:
- Debugging options: `-g`, `-ggdb`, `-ggdb3`, etc.
- Warning options: `-Wall`, `-Wextra`, `-Werror`, `-Wpedantic`, etc.
- Language standard compliance flags: `-std=c11`, `-std=c23`, etc.
- Codegen and architecture tuning options: `-march=native`, `-fstack-protector`, etc.
- Unsupported warning suppression/enablement flags: `-Wno-...`, `-W...`, etc.

This allows build configurations calling standard compiler flags (e.g., `CFLAGS="-O2 -g -Wall"`) to compile using `stricc` without modification.

---

## Build System Integration

Because `stricc` mimics GCC's flags and parameters, you can plug it into existing project build setups.

### Makefile / Autotools
Override the standard `CC` compiler environment variable:
```bash
CC=/path/to/stricc CFLAGS="-O3 -Wall" make
```

### CMake
Configure your CMake build tree by specifying `stricc` as the primary C compiler:
```bash
cmake -DCMAKE_C_COMPILER=/path/to/stricc ..
```
Alternatively, set the environment variable before running cmake:
```bash
export CC=/path/to/stricc
cmake ..
```

---

## Running with Docker (Alternative Setup)

If you want to try `stricc` without installing LLVM 18, Rust, or other build tools on your local system, you can use the provided [Dockerfile.run](../Dockerfile.run). This builds a lightweight sandbox container containing the compiler and all runtime dependencies.

### 1. Build the image
```bash
docker build -f Dockerfile.run -t stricc-sandbox .
```

### 2. Verify the installation
```bash
docker run --rm stricc-sandbox stricc --help
```

### 3. Compile and run local C programs
Mount your current directory into the container to compile C files using `stricc`. Note that since `stricc` targets a custom safe subset of C, platform-specific header includes (like `<stdio.h>`) are not directly supported; instead, declare functions like `printf` manually:
```bash
# Create a test file
echo -e 'int printf(const char *format, ...);\nint main() { printf("Hello from stricc inside Docker!\\n"); return 0; }' > test.c

# Compile the file
docker run --rm -v "$(pwd)":/src stricc-sandbox stricc -o test test.c

# Run the binary
docker run --rm -v "$(pwd)":/src stricc-sandbox ./test
```

---

## Running Tests

To verify correctness of the compiler driver and target output generation, run:

```bash
# Run all workspace unit tests and integration tests
cargo test --workspace

# Run integration safety checks specifically
cargo test --package stricc --test runner
```
