# Transitioning to stricc (Transition Guide)

This guide is designed for developers and junior engineers moving standard C codebases to the safe `stricc` compiler. Because `stricc` prioritizes security and safety, it enforces strict compile-time checks and runtime mitigations that standard compilers (like GCC) ignore.

Here are the most common compilation and runtime safety traps you will encounter, and how to resolve them.

---

## 1. Escaping Stack References (Stack Use-After-Free)

In standard C, returning the address of a local variable or assigning it to a pointer outside its scope is allowed but causes silent memory corruption at runtime. `stricc` rejects this at compile-time.

### Unsafe C Code
```c
int* get_data() {
    int temp = 42;
    return &temp; // Unsafe: temp is destroyed when the function returns
}
```

### stricc Compile Error
```ansi
Error: reference to stack variable 'temp' escapes scope
  At: main.c:3:12
```

### Safe stricc C Code
To fix this, allocate the variable on the **heap** or pass a destination buffer pointer from the caller:
```c
#include <stdlib.h>

int* get_data() {
    int *data = (int *)malloc(sizeof(int));
    *data = 42;
    return data;
}
```

---

## 2. Reaching End of Non-Void Function without Return

Standard compilers compile functions even if some paths fail to return a value, causing undefined behavior if the caller uses the output. `stricc` guarantees all paths return a value.

### Unsafe C Code
```c
int get_discount(int price) {
    if (price > 100) {
        return 10;
    }
    // Bug: price <= 100 returns nothing!
}
```

### stricc Compile Error
```ansi
Error: control path reaches end of non-void function
  At: main.c:6:1
```

### Safe stricc C Code
Ensure all control flow branches return a value, or call a diverging function like `exit()` or `abort()`:
```c
int get_discount(int price) {
    if (price > 100) {
        return 10;
    }
    return 0; // Fixed
}
```

---

## 3. Implicit Pointer to `void *` Conversions

In standard C, pointers are implicitly cast to and from `void *`. Because `stricc` maintains strict type matching constraints, assigning a typed pointer to `void *` (or vice-versa) can result in a compile error.

### Unsafe C Code
```c
int *buf = malloc(10 * sizeof(int));
free(buf); 
```

### stricc Compile Error
```ansi
Error: Incompatible types in assignment: cannot assign Pointer(Int) to Pointer(Void)
  At: main.c:2:10
```

### Safe stricc C Code
Always add explicit typecasts when converting to or from generic pointers (`void *`):
```c
#include <stdlib.h>

int *buf = (int *)malloc(10 * sizeof(int)); // Explicit cast from void*
free((void *)buf);                         // Explicit cast to void*
```

---

## 4. Forbidden Features (Inline Assembly & setjmp)

Features that bypass the compiler's safety checks (such as inline assembly or non-local jumps) are rejected by default in safe mode.

### Unsafe C Code
```c
void run_asm() {
    __asm__("nop"); // Forbidden
}
```

### stricc Compile Error
```ansi
Error: Inline assembly is forbidden in Safe C mode
  At: main.c:2:5
```

### Safe stricc C Code
Remove inline assembly or split the code so that hardware interaction is handled in standard assembly files linked during the build phase.

---

## 5. Lock Safety & Mutex Annotations

`stricc` implements thread safety analysis. If you share variables across threads without qualifying them as `_Atomic`, the compiler checks that they are accessed only under mutex protection.

### Unsafe C Code
```c
int counter = 0; // Shared variable

void* worker(void* arg) {
    counter++; // Error: unsynchronized thread access
    return NULL;
}
```

### Safe stricc C Code
Annotate the shared variable or use C11 atomics:
```c
#include <threads.h>

_Atomic int counter = 0; // Safe atomic operation

void* worker(void* arg) {
    counter++; // Compiled as safe atomic increment
    return NULL;
}
```
