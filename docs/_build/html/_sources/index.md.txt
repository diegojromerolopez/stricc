# stricc Documentation

Welcome to the documentation for **stricc**, a safe, drop-in compiler for a subset of the C programming language that completely eliminates **Undefined Behavior (UB)** at compile-time or via deterministic runtime traps.

Written in Rust and utilizing LLVM (`inkwell`) for optimizing code generation, `stricc` is designed to bridge the gap between C's raw power and modern safety expectations.

## Core Pillars of stricc

- **Safety First**: Zero Undefined Behavior. If compiler analysis cannot prove safety statically (such as array indices or variable lifetimes), safe runtime checks are generated to abort execution cleanly.
- **GCC Flag Compatibility**: Acts as a drop-in replacement for standard C compilers, mimicking standard `gcc` flags for integration with existing build systems (like Make and CMake).
- **Modern C Standard Support**: Designed around a modern C23-like dialect baseline, including support for `nullptr`, `auto`, `constexpr`, and default zero-initialization of variables.
- **ABI & Layout Compatibility**: Operates with standard 8-byte pointers, physical structure alignments, and standard libc interoperability, allowing links to precompiled external libraries.

---

```{toctree}
:maxdepth: 2
:caption: Contents:

cli_usage
ub_mitigation
differences
```
