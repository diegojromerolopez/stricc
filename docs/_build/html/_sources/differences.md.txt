# Differences with GCC / Clang

While `stricc` is designed to be a drop-in replacement for standard C compilers, its core architecture differs significantly from GCC and Clang. This page highlights those differences in terms of compilation philosophy, ABI layout compatibility, performance, diagnostics, and disallowed language features.

---

## 1. Safety vs. Exploitative Optimizations

Traditional C compilers are designed for maximum raw performance. They exploit the C standard's undefined behaviors by assuming they never occur, enabling the optimizer to simplify branches or remove code.

`stricc` shifts the priority to **safety and predictability**:
- Standard compilers assume a signed overflow cannot occur and optimize based on that assumption. `stricc` inserts integer checks and aborts execution on overflow.
- Standard compilers compile array accesses blindly. `stricc` intercepts accesses and validates bounds.
- If a program has a memory safety bug, GCC/Clang may result in a silent exploit or random segmentation fault. `stricc` aborts cleanly, reporting the exact code line responsible.

---

## 2. ABI & Layout Compatibility

Many "Safe C" dialects (like Checked C or Cyclone) introduce fat pointers (pointers bundled with size and bounds metadata). While effective, fat pointers break the binary layout compatibility of structures and function signatures, making it difficult to link against precompiled libraries.

`stricc` maintains **full ABI compatibility**:
- **Standard 8-Byte Pointers**: All pointers remain standard 64-bit hardware memory addresses. Structures have the exact same sizes, paddings, and alignments as they would under GCC or Clang.
- **FFI Boundary Handling**: When calling precompiled libraries (like standard system `libc`), pointers are passed directly. `stricc` assigns an "infinite" boundary metadata (`base = 0`, `size = usize::MAX`) and a wildcard key to any pointer incoming from un-instrumented FFI interfaces.
- **Libc Shims**: The `libstricc_rt` runtime provides wrapper shims for standard allocation functions (`malloc`, `calloc`, `realloc`). When these are called, `stricc` intercepts them and populates the shadow metadata table with the allocated size.

---

## 3. Performance Overhead Mitigation

Because `stricc` instruments pointer dereferences, heap allocations, and arithmetic operations, it introduces some runtime overhead. To keep this overhead to a minimum, `stricc` implements safety-specific optimizations:

- **Value Range Propagation (VRP)**: The compiler statically tracks variable ranges and loop boundaries. If index math proves an array access is guaranteed to be within bounds, the compiler omits the runtime check code, yielding zero-overhead accesses.
- **Link-Time Optimization (LTO)**: `stricc` compiles modules to LLVM bitcode and runs whole-program interprocedural analysis during the final link phase. This allows the compiler to hoist bounds checks out of loops or prune redundant checks across compilation units.

---

## 4. Diagnostics & Developer Experience (DX)

A segmentation fault (`SIGSEGV`) in standard C does not explain why the access was invalid or where it originated. `stricc` provides developer-friendly diagnostics:

- **Compile-time Errors**: Errors like missing returns, stack pointer escapes, or unannotated thread sharing are printed with Rust-like colored labels and caret markers.
- **Symbolicated Runtime Backtraces**: Every generated safety check embeds LLVM DILocation metadata pointing to the C source line and column. If a check fails at runtime, `stricc` walks the stack, parses DWARF symbols, and prints a clean backtrace.

---

## 5. Disallowed Language Features

To guarantee safety, `stricc` restricts or bans features that bypass the compiler's safety analyses:

- **Inline Assembly**: The `__asm__` and `asm` keywords are banned in safe mode.
- **Non-local Jumps (`<setjmp.h>`)**: Because `setjmp` and `longjmp` bypass lexical scope bounds and stack frame teardown analysis, they are rejected.
- **Unannotated Multithreading**: Unsynchronized, concurrent access to variables is blocked. Multi-threaded variables must be qualified with C11 `_Atomic` or guarded by compiler-annotated mutexes, verified by the compiler's lock analyzer.
