# Undefined Behavior Mitigation & Examples

Standard C compilers (like GCC and Clang) treat **Undefined Behavior (UB)** as opportunities for aggressive optimizations. They assume that UB never occurs in a program, and can optimize away safety checks, branch conditions, or entire functions if they contain UB.

`stricc` takes the opposite approach: **all Undefined Behavior is eliminated**. It is either statically checked and rejected at compile-time, defined cleanly to have deterministic behavior, or guarded by runtime checks that trap/abort execution before memory corruption or invalid calculations can occur.

Below is the complete Undefined Behavior mitigation specification for `stricc`, grouped by handling mechanism, followed by a detailed analysis and code examples for major undefined behaviors.

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
| **Infinite Loops without Side Effects** | **Guaranteed Infinite Execution** | The compiler preserves loop control flow in LLVM IR, ensuring infinite loops cannot be optimized away even if they contain no side effects. |

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
| **Struct/Union Member Bounds Bypass** | **Sub-Object Bounds Verification** | Resolves address boundaries of member fields or sub-objects inside struct/unions to prevent nested out-of-bounds bypasses. |
| **Arithmetic Operators Escape** | **Increment/Decrement Bounds Check** | Implements proper pointer metadata propagation and integer overflow checks during prefix/postfix `++` and `--` operations. |
| **Null Dereference on Fallback Pointers** | **Wildcard Null Pointer Validation** | Injects an explicit `is_null` verification check inside bounds checking wrappers to intercept unregistered fallback null returns. |

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

## 1. Spatial Bounds Violations (Out-of-Bounds Access)

In standard C, reading or writing outside the bounds of an array is undefined. It silently corrupts memory (stack variables, heap structures, or function return addresses), which is the primary cause of buffer overflow exploits.

### `stricc` Mitigation
`stricc` maintains standard 8-byte pointer compatibility but maps pointer values to a decoupled **Shadow Metadata Table** containing `(base, size, key)` bounds. Every pointer dereference is instrumented with dynamic checks ensuring `ptr >= base` and `ptr + size_of_access <= base + size`.

### Code Example
```c
#include <stdio.h>

void fail_bounds() {
    int arr[5] = {1, 2, 3, 4, 5};
    // GCC/Clang: Silently overwrites stack frames (dangerous)
    // stricc: Detects the out-of-bounds offset and aborts immediately
    arr[10] = 42; 
}
```

### `stricc` Abort Diagnostics
```text
stricc: runtime check failed (Spatial Safety Bounds Violation)
  Access: Write to 0x7ffd9a10bc28 (offset 40 bytes from base 0x7ffd9a10bc00)
  Valid Range: 20 bytes [0x7ffd9a10bc00 to 0x7ffd9a10bc14]
  At: fail_bounds (src/main.c:7:13)
```

---

## 2. Temporal Safety Violations (Use-After-Free)

Once heap memory is released via `free()`, access to that address (dangling pointer) is undefined in C. Standard compilers may reallocate the block, leading to silent data leaks or remote code execution.

### `stricc` Mitigation
Every heap allocation generates a unique 64-bit version key stored in a global **Shadow Key Map**. The allocated pointer's metadata contains this key. On pointer dereference or deallocation, the key in the pointer's metadata is checked against the global allocation map. When `free()` is called, the key is revoked. Any subsequent access fails.

### Code Example
```c
#include <stdlib.h>
#include <stdio.h>

void fail_uaf() {
    int *ptr = malloc(sizeof(int) * 10);
    ptr[0] = 100;
    free(ptr);
    // GCC/Clang: Accesses dangling memory, leading to potential exploits
    // stricc: Compares pointer key with shadow map and traps instantly
    printf("%d\n", ptr[0]);
}
```

### `stricc` Abort Diagnostics
```text
stricc: runtime check failed (Temporal Safety Key Mismatch)
  Access: Read from 0x55d0f110c200 (Use-After-Free or Double-Free)
  At: fail_uaf (src/main.c:10:20)
```

---

## 3. Integer Division by Zero

Dividing or modulo-ing an integer by zero is undefined in C, often resulting in hard CPU crashes (e.g., `SIGFPE` signals) or unpredictable optimizer assumptions.

### `stricc` Mitigation
`stricc` inserts a safety comparison check before every division or modulo instruction. If the divisor evaluates to `0`, the compiler branches to the runtime abort routine, printing a symbolicated traceback.

### Code Example
```c
int divide(int a, int b) {
    // b = 0 will trigger the division safety check trap
    return a / b; 
}
```

### `stricc` Abort Diagnostics
```text
stricc: runtime check failed (Division/Modulo by Zero)
  Attempted to divide by zero in function 'divide'
  At: src/math.c:3:12
```

---

## 4. Signed Integer Overflow

In standard C, signed integer overflow is undefined (e.g., adding `1` to `INT_MAX`). Standard compilers exploit this to optimize away checks like `x + 1 > x` to `true`.

### `stricc` Mitigation
`stricc` compiles signed arithmetic operations using LLVM overflow-checking intrinsics (e.g., `@llvm.sadd.with.overflow`). If an overflow flag is raised, it traps immediately at runtime. Unsigned arithmetic overflows are defined to wrap cleanly using two's complement modulo behavior.

### Code Example
```c
#include <limits.h>

int overflow_check() {
    int max = INT_MAX;
    // stricc: Traps at runtime instead of wrapping or optimizing out the addition
    return max + 1; 
}
```

---

## 5. Reading Uninitialized Variables

Reading uninitialized local or heap variables in C yields stack/heap garbage, causing information disclosure, logic bugs, or undefined compiler pathways.

### `stricc` Mitigation
`stricc` automatically initializes all local variables, globals, pointers, and structs to zero-filled default states (e.g., integers to `0`, pointers to `nullptr`, character arrays to empty string structures) at declaration. If **Definite Assignment Analysis** proves a variable is always written to before it is read, this initialization is optimized away.

### Code Example
```c
#include <stdio.h>

void print_value() {
    int x; // Uninitialized in standard C
    // GCC/Clang: Prints random stack garbage
    // stricc: Guarantees x is zero-initialized and prints 0
    printf("%d\n", x); 
}
```

---

## 6. Out-of-Bounds Bit Shifts

Shifting an integer by a negative number or by a count equal to or greater than its bit-width (e.g., shifting a 32-bit integer by 35) is undefined in C.

### `stricc` Mitigation
`stricc` mask-limits all dynamic shift amounts. For a 32-bit integer, the shift count is masked with `31` (i.e., `shift_count & 31`), turning out-of-bounds shifts into deterministic modulo shifts. If the shift count is a constant, out-of-bounds values are caught at compile-time.

### Code Example
```c
int shift_modulo(int val, int count) {
    // If count = 35, standard C is undefined.
    // stricc performs: val << (35 & 31) -> val << 3
    return val << count; 
}
```

---

## 7. Reaching End of Non-Void Function without Return

If execution flows to the closing brace of a value-returning function without returning a value, calling it and using its value is undefined.

### `stricc` Mitigation
`stricc` performs strict **Definite Return Analysis** on the AST. If any control-flow path can exit a non-void function without a `return` statement or a diverging call (like `exit` or `abort`), compile-time analysis rejects the program.

### Code Example
```c
int fail_return(int x) {
    if (x > 10) {
        return 1;
    }
    // GCC/Clang: Compiles with a warning, returning garbage if x <= 10.
    // stricc: Fails to compile: "error: control path reaches end of non-void function"
}
```

---

## 8. Stack Variable Escapes (Stack Use-After-Free)

Returning the address of a local variable or assigning it to a pointer declared in an outer scope creates a dangling stack reference. Accessing it after the scope exits is undefined.

### `stricc` Mitigation
`stricc` performs static compile-time **Lifetime & Escape Analysis**. The compiler tracks lexical scopes and rejects any attempt to pass or assign stack-allocated addresses to a reference that outlives them.

### Code Example
```c
int* fail_escape() {
    int temp = 42;
    // stricc: Statically rejects compilation
    // "error: reference to stack variable 'temp' escapes scope"
    return &temp; 
}
```
