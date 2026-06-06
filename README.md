# `stricc`

[![CI](https://github.com/diegojromerolopez/stricc/actions/workflows/ci.yml/badge.svg)](https://github.com/diegojromerolopez/stricc/actions/workflows/ci.yml)
[![C Language](https://img.shields.io/badge/C-00599C?style=flat-square&logo=c&logoColor=white)](https://en.wikipedia.org/wiki/C_(programming_language))
[![C Standard](https://img.shields.io/badge/Standard-C23%2B-00599C?style=flat-square&logo=c&logoColor=white)](https://en.wikipedia.org/wiki/C23_(C_standard_revision))
[![LLVM Backend](https://img.shields.io/badge/LLVM-18-red?style=flat-square&logo=llvm&logoColor=white)](https://llvm.org/)
[![Memory Safety](https://img.shields.io/badge/Memory--Safety-Guaranteed-success?style=flat-square)](https://github.com/diegojromerolopez/stricc)
[![Compiler Written in Rust](https://img.shields.io/badge/Written%20in-Rust-black?style=flat-square&logo=rust&logoColor=white)](https://www.rust-lang.org/)

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
