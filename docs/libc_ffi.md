# Standard Library Shims & FFI Boundaries

When building C programs, you regularly interact with the Standard C Library (libc) and external third-party libraries (e.g. databases, compression libraries). 

For a safe compiler like `stricc`, this creates a unique challenge: **how do we enforce safety checks on pointers when interacting with code we did not compile?**

This document explains the Foreign Function Interface (FFI) strategy and the pre-instrumented standard library shims built into `stricc`.

---

## 1. What is the FFI Boundary Challenge?

In computer science, **Foreign Function Interface (FFI)** refers to the boundary where code written in one language or compiled under one system calls code compiled elsewhere.

When a `stricc` program passes a pointer to an external precompiled library (like `sqlite3`), or receives a pointer back (like `getenv` returning a string from the OS environment):
* The external library **does not check shadow metadata** (since it wasn't compiled with `stricc`).
* The external library **does not register its allocations** in our shadow metadata table.

If `stricc` strictly enforced bounds checking on these external pointers, the program would immediately crash with a safety check violation, even though the pointer is valid.

---

## 2. How stricc Handles FFI Boundaries

`stricc` employs two techniques to maintain compatibility with external libraries while keeping your code safe:

### A. Wildcard/Infinite Fallback Metadata
When a pointer is loaded from memory that has no registered shadow metadata (indicating it came from external FFI code), `__stricc_rt_shadow_load` returns:
* `base = 0` (or `nullptr`)
* `size = usize::MAX` (maximum possible memory size)
* `key = 0` (wildcard version key)

This designates the pointer as a **wildcard pointer**, bypassing standard bounds/UAF checks on that specific address and preventing false-positive crashes when interacting with host system memory.

### B. Safe Libc Shims
For standard C library functions, `stricc`'s runtime library (`libstricc_rt`) intercepts calls and routes them through **instrumented wrappers**. These wrappers query shadow metadata to enforce safety before delegating to the host library:

| Libc Area | Function Wrapper | Safe Instrumentation & Fixes |
| :--- | :--- | :--- |
| **Allocation** | `malloc`, `calloc`, `realloc`, `free` | Generates and verifies unique 64-bit temporal keys. Restores original size on `realloc` failure. |
| **String Ops** | `strlen`, `strcpy`, `strncpy`, `strcmp` | Queries shadow bounds to verify a null-terminator `\0` exists within the allocation before reading. |
| **Memory Copy** | `memcpy`, `memset` | Automatically handles overlapping buffers as `memmove` under the hood. Prevents crashes on `NULL` with size `0`. |
| **Character Ops** | `isalpha`, `isdigit`, `tolower`, etc. | Asserts that input integer arguments reside strictly within the valid range `[-1, 255]`. |
| **Alignment** | `aligned_alloc` | Verifies that requested alignment is a power of 2 and size is a multiple of alignment. |

---

## 3. Explicit FFI Boundaries (Contributor Note)

When developing libraries or bindings for `stricc`, you can assist the compiler's static analysis by marking functions that interface with raw system pointers using the compiler attribute:

```c
__attribute__((ffi_boundary)) void process_raw_buffer(char *raw_ptr);
```

This instructs the compiler's typechecker and codegen to:
- Treat `raw_ptr` as incoming from an un-instrumented boundary.
- Automatically associate wildcard infinite metadata to it on function entry.
- Prevent compile-time type warnings when casting it to specific internal layouts.
