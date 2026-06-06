# `stricc`

A safe, drop-in compiler for a subset of the C programming language that completely eliminates **Undefined Behavior (UB)** at compile-time or via deterministic runtime traps.

Written in Rust, utilizing LLVM (`inkwell`) for optimizing code generation, `stricc` is designed to bridge the gap between C's raw power and modern safety expectations.

> [!NOTE]
> Read the full architectural specifications and implementation details in [PLAN.md](file:///Users/diegoj/repos/stricc/PLAN.md).

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

## Core Technologies

* **Shadow Metadata (SoftBound+CETS style)**: Retains standard 8-byte pointer layouts, ensuring full ABI, structure layout, and calling convention compatibility with GCC/Clang compiled code. This allows standard system headers (`<stdio.h>`) to be parsed directly and linked with system libraries.
* **Link-Time Optimization (LTO)**: Compiles source code to LLVM bitcode and enables LTO by default. This permits global interprocedural analysis to prune redundant bounds and shadow check lookups.
* **Value Range Propagation (VRP)**: Performs static analysis of loop induction variables and integer ranges to reject out-of-bound errors at compile-time and strip unnecessary runtime checks.
* **Rust & LLVM**: Built in Rust for robust development, leveraging LLVM for state-of-the-art optimizer and target codegen.

---

## Getting Started & Usage

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

# Output LLVM IR representation
./target/release/stricc --emit-llvm -o main.ll main.c

# Compile with macros and include paths
./target/release/stricc -I./include -DDEBUG=1 -o app main.c
```

### 4. Running Tests
Run the entire conformance and safety matrix test suite:
```bash
# Run all workspace unit tests and integration tests
cargo test --workspace

# Run integration safety checks specifically
cargo test --package stricc --test runner
```
