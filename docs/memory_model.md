# The stricc Memory Safety Model

This document explains how `stricc` guarantees spatial and temporal memory safety under the hood without breaking compatibility with standard C structures and compiled libraries.

---

## 1. Physical Pointer Representation (ABI Compatibility)

In many research languages or safety extensions (like Checked C), pointers are expanded into "fat pointers" containing bounds metadata:

```text
Unsafe C Pointer (8 bytes):    [   Address (64-bit)   ]
Checked C Fat Pointer (16B):   [ Address ] [ Base ] [ Size ]
```

While fat pointers make bounds-checking easy, they change the physical layout of structures and function signatures. This makes it impossible to link your compiled binary with external precompiled libraries (like standard system `libc` or `sqlite3`).

### The stricc Strategy
`stricc` maintains **standard 8-byte pointers**. To the OS and CPU, `stricc` pointers look exactly like standard GCC or Clang pointers. We track bounds and keys **outside** the program's normal memory space.

---

## 2. Pointer Metadata Structure

Every pointer in a `stricc` program has associated metadata:

```rust
struct PointerMetadata {
    base: usize, // Start address of the valid memory block
    size: usize, // Total size of the allocation in bytes
    key: u64,    // Temporal safety version key
}
```

---

## 3. How Metadata is Stored and Tracked

`stricc` uses two distinct strategies depending on where the pointer lives:

### A. Local Pointers (Registers and Stack)
For pointers that live in local variables or CPU registers, the compiler generates separate LLVM registers/stack slots for their `base`, `size`, and `key`. 
- **Benefit**: Zero-overhead metadata propagation. No database or memory lookups are needed to trace local pointer variables.

### B. Pointers in Memory (Structs and Globals)
When a pointer is stored inside a struct, an array, or a global variable, its metadata must be written to the **Shadow Metadata Table**.

- **Actual Implementation**: The runtime library (`libstricc_rt`) maintains a thread-safe, global `HashMap<usize, Metadata>` where the key is the address of the pointer variable in memory (`&ptr_val`), and the value is the `(base, size, key)` tuple.
- **Example Flow**:
  1. A pointer `p` is stored inside a struct field `s.p = malloc(10);`.
  2. The compiler generates a call to `__stricc_rt_shadow_store(&s.p, base, size, key)`.
  3. When `s.p` is loaded back, the compiler generates a call to `__stricc_rt_shadow_load(&s.p, &base_out, &size_out, &key_out)`.

> [!NOTE]
> **Planned Performance Optimization**: For production-grade speed, the hash-map lookup is planned to be replaced with a **Direct Address Mapping** scheme. The shadow address is computed directly from the memory address:
> $$\text{shadow\_addr} = (\text{ptr\_addr} \gg 3) \times \text{sizeof(PointerMetadata)} + \text{ShadowBase}$$
> This avoids lock contention and achieves near-zero lookup overhead.

---

## 4. Temporal Safety (Use-After-Free Prevention)

`stricc` implements version-keyed safety to completely block Use-After-Free (UAF) and Double Free vulnerabilities.

### The Allocation Lifecycle
1. **Allocation (`malloc`)**:
   - The runtime allocates memory using the system allocator.
   - It generates a **new, unique 64-bit key** from a global counter.
   - It inserts the key into the global `KEY_TABLE` mapping the memory block's address to the key.
   - It returns the pointer with its metadata set to `(base_address, size, key)`.
2. **Dereference (`*ptr`)**:
   - The compiler emits a bounds and key check (`__stricc_rt_check_bounds`).
   - The runtime compares the pointer's metadata key against the active key in the `KEY_TABLE` for `base_address`.
   - If they match, the access is allowed.
3. **Deallocation (`free`)**:
   - The runtime removes the key for the memory address from the `KEY_TABLE` and `SHADOW_TABLE`.
   - The memory is returned to the system.
4. **Use-After-Free Attempt**:
   - If the program attempts to access the memory block later using the old pointer, the runtime check fails because the key is no longer in the active key map, resulting in a safe runtime abort.

---

## 5. Control Flow Integrity (CFI)

If a hacker exploits a bug to overwrite a function pointer variable in memory, they can redirect execution flow to arbitrary code blocks.

To prevent this, `stricc` applies Control Flow Integrity (CFI):
- The compiler computes a **unique type signature hash** for every function type defined in your code.
- Functions are tagged with their signature hash.
- Before executing an indirect call (e.g. `fp()`), the compiler emits check code that verifies the target function's signature hash matches the expected signature hash.
- If they do not match, execution is immediately aborted.
