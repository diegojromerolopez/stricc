# `stricc`

[![CI](https://github.com/diegojromerolopez/stricc/actions/workflows/ci.yml/badge.svg)](https://github.com/diegojromerolopez/stricc/actions/workflows/ci.yml)
[![Documentation Status](https://readthedocs.org/projects/stricc/badge/?version=latest)](https://stricc.readthedocs.io/en/latest/?badge=latest)
[![C Language](https://img.shields.io/badge/C-00599C?style=flat-square&logo=c&logoColor=white)](https://en.wikipedia.org/wiki/C_(programming_language))
[![C Standard](https://img.shields.io/badge/Standard-C23%2B-00599C?style=flat-square&logo=c&logoColor=white)](https://en.wikipedia.org/wiki/C23_(C_standard_revision))
[![LLVM Backend](https://img.shields.io/badge/LLVM-18-red?style=flat-square&logo=llvm&logoColor=white)](https://llvm.org/)
[![Memory Safety](https://img.shields.io/badge/Memory--Safety-Guaranteed-success?style=flat-square)](https://github.com/diegojromerolopez/stricc)
[![Compiler Written in Rust](https://img.shields.io/badge/Written%20in-Rust-black?style=flat-square&logo=rust&logoColor=white)](https://www.rust-lang.org/)

A safe, drop-in compiler for a subset of the C programming language that completely eliminates **Undefined Behavior (UB)** at compile-time or via deterministic runtime traps.

Written in Rust, utilizing LLVM (`inkwell`) for optimizing code generation, `stricc` is designed to bridge the gap between C's raw power and modern safety expectations.

> [!NOTE]
> Read the online documentation on [Read the Docs](https://stricc.readthedocs.io/en/latest/) or review the full architectural specifications and implementation details in [PLAN.md](file:///Users/diegoj/repos/stricc/PLAN.md).

> [!WARNING]
> This project has been vibe-coded. Expect experimental features, rapid changes, and potential vibes.

---

## Why `stricc`? (Comparison with GCC)

Unlike standard compilers like GCC and Clang which exploit C's undefined behaviors for optimizations, `stricc` prioritizes execution safety, turning memory vulnerabilities and logic bugs into predictable aborts or compile-time failures.

| Safety Category | Standard GCC / Clang | `stricc` Compiler |
| :--- | :--- | :--- |
| **Spatial Safety** | **Unsafe**: Out-of-bounds array reads and writes corrupt stack/heap memory silently, leading to security exploits. | **Guaranteed**: Uses a **Shadow Metadata Table** mapping pointers to `(base, size, key)`. Dereferences are dynamically bounds-checked. |
| **Temporal Safety** | **Unsafe**: Use-after-free and double-free lead to dangling pointer accesses and heap exploits. | **Guaranteed**: Pointer versions are validated against a global **Shadow Key Map** on dereference and deallocation. |
| **Initialization** | **Unsafe**: Uninitialized reads yield random stack garbage, causing information leaks or logic bugs. | **Guaranteed**: Automatic safe zero-initialization of local/global variables, pointers, and structures. |
| **Undefined Behavior** | **Optimized/Exploited**: Signed integer overflows, out-of-bounds shifts, etc. are treated as unreachable code, deleting checks. | **Eliminated**: All standard C UB is either defined cleanly (e.g. modulo shift masking) or triggers a runtime trap. |
| **Variadic Functions** | **Unsafe**: `<stdarg.h>` provides no type or boundary checks, frequently exploited via format strings. | **Guaranteed**: Variadic arguments are packaged with type descriptor metadata and verified on extraction. |
| **Control Flow Integrity** | **Limited**: Function pointers can be hijacked to execute arbitrary indirect jumps (ROP chains). | **Guaranteed**: Generates Control Flow Integrity (CFI) signature hashes to validate indirect jumps at runtime. |
| **Diagnostics & DX** | **Obfuscated**: Undefined behaviors lead to silent corruptions, or generic `SIGSEGV` core dumps. | **Developer-First**: Aborts print a **DWARF-symbolicated backtrace** showing exact source line, column, and call stack. |

---

## Safety in Action: Code Examples

Here is how `stricc` protects standard C code from typical undefined behavior bugs:

### 1. Spatial Bounds Violations (Stack Overflow / Out-of-Bounds)
```c
#include <stdio.h>

void fail_bounds() {
    int arr[5] = {1, 2, 3, 4, 5};
    // GCC/Clang: Silently overwrites stack frames or local variables (highly exploitable)
    // stricc: Catches the violation and cleanly aborts before memory corruption occurs
    arr[10] = 42; 
}
```
**`stricc` Runtime Output:**
```ansi
stricc: runtime check failed (Spatial Safety Bounds Violation)
  Access: Write to 0x7ffd9a10bc28 (offset 40 bytes from base 0x7ffd9a10bc00)
  Valid Range: 20 bytes [0x7ffd9a10bc00 to 0x7ffd9a10bc14]
  At: fail_bounds (src/main.c:7:13)
```

### 2. Temporal Safety Violations (Use-After-Free)
```c
#include <stdlib.h>
#include <stdio.h>

void fail_uaf() {
    int *ptr = malloc(sizeof(int) * 10);
    ptr[0] = 100;
    free(ptr);
    // GCC/Clang: Accesses dangling memory, risking data corruption or double-free exploits
    // stricc: Compares pointer metadata key with shadow allocation map and traps immediately
    printf("%d\n", ptr[0]);
}
```
**`stricc` Runtime Output:**
```ansi
stricc: runtime check failed (Temporal Safety Key Mismatch)
  Access: Read from 0x55d0f110c200 (Use-After-Free or Double-Free)
  At: fail_uaf (src/main.c:10:20)
```

---

## Undefined Behavior (UB) Mitigation Specification

The C standard lists numerous undefined behaviors. `stricc` mitigates every standard UB through three distinct mechanisms: **Defined Semantics (Non-Aborting)**, **Controlled Runtime Aborts (Traps)**, and **Compile-Time Rejection (Static Analysis)**.

### 1. Defined Semantics (Non-Aborting)

These behaviors, which are undefined in standard C, are assigned safe, deterministic, non-aborting semantics:

| Standard C Undefined Behavior | `stricc` Defined Semantics | Technical Explanation / Example |
| :--- | :--- | :--- |
| **Uninitialized Variable/Pointer Reads** | **Safe Default Initialization** | Automatically zero-initializes all local/global variables, pointers, and structures to type-specific defaults (e.g., `0` for integers, `nullptr` for pointers, and static `""` empty string literal for character pointers). |
| **Out-of-Bounds Bit Shifts** | **Modulo Shift Masking** | Shifts are restricted by bitwise-masking the shift amount to the bit-width of the operand: `shift_count & (bit_width - 1)`. For example, a shift by `35` on a 32-bit integer is safely resolved as a shift by `3`. |
| **Overlapping Memory Copy (`memcpy`)** | **Safe Copying (`memmove` Fallback)** | Overlapping buffers passed to `memcpy` are automatically handled as a `memmove` under the hood, preventing memory overlap corruptions. |
| **`NULL` Arguments to `memcpy` / `memset`** | **Safe No-Op on Zero Size** | Passing `NULL` to library memory/string functions is defined as a safe no-op if the size parameter `n` is `0` (e.g., `memcpy(NULL, NULL, 0)` executes safely without trapping). |
| **Strict Aliasing Violations** | **Defined Type Punning** | Type punning is fully defined. `stricc` disables strict-aliasing optimization assumptions (behaves like `-fno-strict-aliasing`), reading and writing the raw bit patterns in memory. |
| **Union Active Member Mismatch** | **Defined Bitcast** | Reading an inactive union member is defined to yield the bitcast/reload representation of the underlying active member's bits. |
| **Cross-Allocation Pointer Comparisons** | **Defined Total Ordering** | Relational comparisons (`<`, `>`, `<=`, `>=`) on pointers from different allocations compare their absolute virtual address values. |
| **Incorrect Use of `restrict`** | **Defined Aliased Memory** | The `restrict` keyword is parsed but completely ignored during code generation, preventing LLVM from optimizing under incorrect aliasing assumptions. |
| **Evaluation Order & Sequence Points** | **Defined Left-to-Right Evaluation** | Operand evaluations and function arguments are evaluated strictly from left to right (e.g., in `f(g(), h())`, `g()` is guaranteed to execute before `h()`). |
| **Invalid `bool` Value (Trap Representation)** | **Guaranteed Normalization** | Loaded boolean values are forced to be sanitized/normalized using a `!= 0` check in LLVM IR, preventing invalid bit representations. |
| **Floating-Point Overflow** | **Defined IEEE-754 Semantics** | Floating-point operations strictly follow IEEE-754 rules, cleanly producing `INFINITY` or `NaN` without undefined compiler assumptions. |
| **Unsigned Integer Overflow** | **Modulo Wrapping** | Wraps deterministically using two's complement modulo arithmetic. |

### 2. Controlled Runtime Aborts (Traps)

For behaviors that threaten memory or execution safety, `stricc` emits runtime checks that immediately abort execution safely and print a DWARF-symbolicated backtrace instead of allowing silent corruption:

| Standard C Undefined Behavior | `stricc` Safe Mitigation | Implementation / Error Output |
| :--- | :--- | :--- |
| **Out-of-Bounds Memory Access** | **Spatial Safety Bounds Check** | Tracks pointer ranges dynamically using a shadow metadata table. Dereferences assert that `base <= ptr < base + size`. |
| **Use-After-Free & Double Free** | **Temporal Safety Key Check** | Generates unique keys for allocations. Frees and dereferences assert that the pointer's key matches the current live allocation key. |
| **Null Pointer Dereference** | **Guaranteed Pointer Null Check** | Intercepted by shadow checks (as null pointers map to size `0` in shadow metadata) or explicit validation before wildcard returns. |
| **Division / Modulo by Zero** | **Controlled Division Zero Check** | Injects checks before division instructions. Aborts with a clear diagnostic if the divisor is `0`. |
| **Integer Division Overflow** | **Controlled Overflow Check** | Aborts cleanly on signed arithmetic overflows such as `INT_MIN / -1` or `INT_MIN % -1`. |
| **Signed Integer Overflow** | **Controlled Overflow Trap** | Promotes math operations to LLVM intrinsics with overflow flags (e.g. `@llvm.sadd.with.overflow`). Aborts on overflow by default. |
| **Float-to-Int Conversion Overflow** | **Conversion Range Check** | Emits bounds validation checks before converting floats/doubles to integers, aborting if the value is out of bounds. |
| **Unaligned Memory Access** | **Alignment Verification** | Verifies pointer alignment using the target type's natural alignment before dereference. |
| **Mismatched Function Pointer Call** | **Control Flow Integrity (CFI)** | Validates indirect function pointer calls against a unique type signature hash before jump execution. |
| **Stack Overflow** | **Stack Clash Protection** | Compiles with stack probes (`-fstack-clash-protection`) and traps guard-page faults cleanly via a custom signal handler. |
| **Invalid VLA Size** | **VLA Size Check** | Asserts that variable-length array sizes are strictly greater than `0` before dynamic stack allocation. |
| **Non-Null-Terminated String Library Inputs** | **String Bounds Verification** | Wraps library calls (`strlen`, `strcpy`, etc.) to query shadow size and verify a null terminator exists within the allocation. |
| **ctype Library Out-of-Range Arguments** | **ctype Arguments Check** | Wraps `ctype` functions (`isalpha`, `isdigit`, etc.) to assert that arguments reside within `[-1, 255]`. |
| **Invalid Allocation Alignment** | **aligned_alloc Bounds Check** | Wraps `aligned_alloc` to assert that alignment is a valid power of 2 and size is a multiple of alignment. |
| **Modifying Const / String Literals** | **Write Safety Verification** | Stored in read-only segments (`.rodata`) and key-mapped in shadow metadata to trigger hardware page faults or write traps. |
| **Failed realloc Size Restoration** | **realloc Size Restoration** | Restores the original shadow size of the pointer if a `realloc` fails and returns `NULL`, preventing subsequent out-of-bounds bypasses. |

### 3. Compile-Time Rejections (Static Analysis)

Certain behaviors are prevented entirely by the compiler frontend, which rejects compilation with clear diagnostic messages:

| Standard C Undefined Behavior | `stricc` Prevention | Detection Phase / Mechanism |
| :--- | :--- | :--- |
| **Reaching End of Non-Void Function** | **Definite Return Analysis** | Semantic analyzer verifies that all control-flow paths return a value or diverge (e.g. call `abort()`). |
| **Stack Use-After-Free / Escaping Stack** | **Escape & Lifetime Analysis** | Lexical scope lifetime analysis rejects returning addresses of stack-allocated variables or assigning them to outer-scope pointers. |
| **Inline Assembly & setjmp/longjmp** | **Forbidden Features Rejection** | Disallows `__asm__`, `setjmp`, and `longjmp` keyword usage in safe compilation mode. |
| **Multithreading Data Races** | **Thread-Safety & Lock Verification** | Statically rejects unannotated multithreading code, enforcing `_Atomic` qualifiers or checking static mutex-lock annotations. |
| **Format String Mismatches** | **Format Specifier Type Checker** | Statically type-checks format arguments against string specifiers for constant formatting literals. |
| **Link-Time Incompatible Globals** | **Interprocedural Type Validation** | Compares global variable types across translation units during Link-Time Optimization (LTO) and rejects mismatching linkages. |
| **Keyword Redefinition** | **Preprocessor Macro Restrictions** | Rejects macro definitions attempting to redefine language keywords (e.g., `#define int double`) or reserved identifiers. |
| **Local Block-Scope Variable Escape** | **Block Scope Escape Analysis** | Detects and blocks local variables whose addresses escape their immediate defining scope. |

---

## Technical Architecture & ABI Compatibility

One of the biggest concerns C developers have when using "Safe C" dialects (like Checked C) is **compatibility** and **performance overhead**. 

### 1. Full ABI & Structure Layout Compatibility
`stricc` maintains strict binary compatibility with GCC and Clang compiled code:
* **Standard 8-Byte Pointers**: Pointers are physical 64-bit addresses, NOT fat structures. Struct layouts, field alignments, and function signatures remain identical to standard C.
* **Link with Precompiled Libraries**: You can safely link object files compiled with `stricc` against precompiled libraries (such as `libz.a`, `sqlite3.o`, or standard system `libc`).
* **Interoperable FFI**: Functions taking pointers can be passed to assembly or external libraries. `stricc` assigns "infinite" wildcard metadata bounds to pointers incoming from un-instrumented FFI boundaries.

### 2. High-Performance Safety Optimization
* **Value Range Propagation (VRP)**: `stricc` analyzes loop boundaries and index ranges statically. When an index can be mathematically proven to be safe, the compiler **suppresses the generation of runtime bounds checks**, achieving zero-overhead.
* **Link-Time Optimization (LTO)**: Enabling LTO allows `stricc` to perform interprocedural analysis across compilation units, hoisting or pruning redundant shadow metadata accesses globally.

---

## Getting Started & Usage

> [!TIP]
> For the full list of compiler flags, detailed usage tutorials, and advanced compiler configurations, visit the [stricc CLI Usage & Integration Guide](https://stricc.readthedocs.io/en/latest/cli_usage.html) on Read the Docs.

### Running with Docker (Alternative Setup)

If you want to try `stricc` without installing LLVM 18, Rust, or other build tools on your local system, you can use the provided [Dockerfile.run](file:///Users/diegoj/repos/stricc/Dockerfile.run). This builds a lightweight sandbox container containing the compiler and all runtime dependencies.

1. **Build the image**:
   ```bash
   docker build -f Dockerfile.run -t stricc-sandbox .
   ```

2. **Verify the installation**:
   ```bash
   docker run --rm stricc-sandbox stricc --help
   ```

3. **Compile and run local C programs**:
   Mount your current directory into the container to compile C files using `stricc`. Note that since `stricc` targets a custom safe subset of C, platform-specific header includes (like `<stdio.h>`) are not directly supported; instead, declare functions like `printf` manually:
   ```bash
   # Create a test file
   echo -e 'int printf(const char *format, ...);\nint main() { printf("Hello from stricc inside Docker!\\n"); return 0; }' > test.c

   # Compile the file
   docker run --rm -v "$(pwd)":/src stricc-sandbox stricc -o test test.c

   # Run the binary
   docker run --rm -v "$(pwd)":/src stricc-sandbox ./test
   ```

### 1. Prerequisites
`stricc` requires **LLVM 18** to compile and run.
- **macOS**: `brew install llvm@18`
- **Ubuntu/Debian**: Follow instructions on [apt.llvm.org](https://apt.llvm.org/) to install `llvm-18`.

### 2. Building the Project
Build the compiler driver and the runtime support library (`libstricc_rt`):
```bash
cargo build --release
```
This produces the compiler binary at `./target/release/stricc` and the runtime library `libstricc_rt.a` under `./target/release/`.

### 3. Compiling C Code
The `stricc` CLI mimics standard GCC flags:
```bash
# Compile a C file to a safe executable (automatically links libstricc_rt)
./target/release/stricc -o app main.c

# Compile to an object file only (do not link)
./target/release/stricc -c -o helper.o helper.c

# Compile with optimization level 3 and include headers
./target/release/stricc -O3 -I./include -o app main.c
```

### 4. Build System Integration
Since `stricc` mimics GCC flags, you can easily plug it into your existing build tools.

#### Make / Autotools
Simply override the `CC` compiler variable:
```bash
CC=stricc CFLAGS="-O3 -Wall" make
```

#### CMake
Configure your CMake project to use `stricc` as the primary C compiler:
```bash
cmake -DCMAKE_C_COMPILER=/path/to/stricc ..
```

### 5. Running Tests
Run the entire conformance and safety matrix test suite:
```bash
# Run all workspace unit tests and integration tests
cargo test --workspace

# Run integration safety checks specifically
cargo test --package stricc --test runner
```

---

## Developer Guide & Contributing

If you are a newcomer looking to work on the `stricc` compiler itself, this section will help you set up your environment and understand the codebase layout.

### 1. Developer Prerequisites

`stricc` relies on the `inkwell` crate to bind to the LLVM 18 C++ APIs. In addition to installing LLVM 18, your compiler needs to be able to locate the `llvm-config` binary during the build process.

*   **macOS (Homebrew)**:
    Install LLVM 18:
    ```bash
    brew install llvm@18
    ```
    Before running `cargo build` or `cargo test`, set the following environment variables:
    ```bash
    export LLVM_SYS_180_PREFIX="/opt/homebrew/opt/llvm@18"
    export PATH="/opt/homebrew/opt/llvm@18/bin:$PATH"
    ```
*   **Ubuntu/Debian**:
    Ensure LLVM 18 is installed and `llvm-config-18` is in your `PATH` or symlinked as `llvm-config`.

### 2. Codebase & Directory Layout

The workspace is divided into two primary crates: the compiler driver and the runtime support library.

*   **[`stricc/`](file:///Users/diegoj/repos/stricc/stricc)**: The compiler implementation.
    *   [`src/lexer.rs`](file:///Users/diegoj/repos/stricc/stricc/src/lexer.rs): Tokenizes the preprocessed C source code.
    *   [`src/parser.rs`](file:///Users/diegoj/repos/stricc/stricc/src/parser.rs): Hand-written recursive descent parser that parses tokens into the Abstract Syntax Tree (AST).
    *   [`src/ast.rs`](file:///Users/diegoj/repos/stricc/stricc/src/ast.rs): Defines the AST nodes.
    *   [`src/typechecker.rs`](file:///Users/diegoj/repos/stricc/stricc/src/typechecker.rs): Performs type validation, Value Range Propagation (VRP) to optimize bounds checks, escape analysis, and constant folding.
    *   [`src/codegen.rs`](file:///Users/diegoj/repos/stricc/stricc/src/codegen.rs): Translates the AST to LLVM IR using `inkwell` and injects safety checks.
    *   [`src/driver.rs`](file:///Users/diegoj/repos/stricc/stricc/src/driver.rs): Coordinates the compiler stages and manages host preprocessor delegation.
    *   [`src/main.rs`](file:///Users/diegoj/repos/stricc/stricc/src/main.rs): Entrypoint for CLI parsing and flag emulation.
*   **[`runtime/`](file:///Users/diegoj/repos/stricc/runtime)**: The runtime support library (`libstricc_rt.a`).
    *   [`src/lib.rs`](file:///Users/diegoj/repos/stricc/runtime/src/lib.rs): Implements the memory allocation wrappers (`malloc`/`free`), shadow metadata table operations, stack overflow/signal handlers, and symbolicated DWARF backtrace reporting.

Refer to the detailed specification in [PLAN.md](file:///Users/diegoj/repos/stricc/PLAN.md) for architectural guidelines.

### 3. Local Development & Advanced Testing

The root directory contains a `Makefile` that simplifies building and running the extended test suites:

*   **Build the workspace**:
    ```bash
    make build
    ```
*   **Run all unit/integration tests**:
    ```bash
    make test
    ```
*   **Run the GCC C Torture Suite** (compares outputs against GCC for ~1,500 test cases):
    ```bash
    make test-gcc
    ```
*   **Run the LLVM Test Suite**:
    ```bash
    make test-llvm
    ```

Refer to [TEST.md](file:///Users/diegoj/repos/stricc/TEST.md) for a complete breakdown of the safety matrix and conformance suites.

---

## Future Improvements

To transition `stricc` from a prototype to a production-grade compiler, the following roadmap of future improvements is planned:

*   **Documentation of Source Code**: Enhance inline code comments, expand Rustdoc documentation for internal compiler API interfaces (lexer, parser, typechecker, and codegen modules), and publish comprehensive design details.
*   **Limitation of Source Code File Size**: Implement strict compiler guards or compiler limits on source file sizes, token counts, and recursion depth to prevent denial-of-service (DoS) style stack exhaustion during parsing and analysis phases.
*   **Ensuring Idiomatic Rust**: Refactor legacy areas of the compiler backend to use idiomatic Rust patterns (e.g., proper error propagation with `Result`/`Option`, zero-copy parsing where feasible, and avoiding unnecessary clones/unwraps).
*   **Performance Metrics & Benchmarking**: Integrate automated benchmarking suites (e.g., using `criterion`) to track compile-time performance, memory consumption of the compiler, and runtime overhead of safety-instrumented executables.
*   **More Tests**: Expand the test suites to include more edge cases, deeper integration tests for complex pointer aliasing, and larger real-world C codebases to improve coverage and reliability.
*   **Alternative Backends & LLVM Removal**: Investigate replacing the LLVM/Inkwell backend with a custom lightweight backend or code generator to reduce compilation dependencies, build times, and runtime binary footprint.



