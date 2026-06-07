# Undefined Behavior Mitigation & Examples

Standard C compilers (like GCC and Clang) treat **Undefined Behavior (UB)** as opportunities for aggressive optimizations. They assume that UB never occurs in a program, and can optimize away safety checks, branch conditions, or entire functions if they contain UB.

`stricc` takes the opposite approach: **all Undefined Behavior is eliminated**. It is either statically checked and rejected at compile-time, defined cleanly to have deterministic behavior, or guarded by runtime checks that trap/abort execution before memory corruption or invalid calculations can occur.

Below is a detailed analysis of major undefined behaviors in standard C and how `stricc` turns them into safe, defined outcomes, along with code examples.

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
