# `stricc` Technical Specification & Architecture

`stricc` is a safe, drop-in compiler for a defined-behavior subset of the C programming language (aligning with C23 features such as `const`, `nullptr`, `auto`, and `constexpr`). Written in Rust and utilizing LLVM 18 (via the `inkwell` crate) for code generation, `stricc` completely eliminates **Undefined Behavior (UB)** at compile-time (via static analysis) or through deterministic runtime traps and two's-complement wrapping.

---

## 1. Primary Goal & Core Technical Invariants

The primary objective of `stricc` is to build a **100% Safe and Completely Deterministic C Compiler**.

> [!IMPORTANT]
> **GOAL 1: ABSOLUTE EXECUTION & MEMORY SAFETY**
> - **Zero Undefined Behavior (UB)**: Any C program compiled with `stricc` MUST have 100% deterministic, safe, and defined behavior.
> - **No Silent Corruption**: Spatial memory violations (out-of-bounds array reads/writes), temporal violations (use-after-free, double-free), null dereferences, and integer overflows MUST NEVER corrupt memory or execute arbitrary behavior. They MUST be rejected at compile-time or trigger a clean, symbolicated runtime abort (`stricc_abort`) with a DWARF stack backtrace.

> [!IMPORTANT]
> **GOAL 2: 100% DETERMINISTIC COMPILATION & EXECUTION**
> - **Deterministic Semantics**: All evaluation orders, shift operations, arithmetic operations, type casts, and memory operations yield identical, 100% repeatable results across every execution run, CPU target, and optimization level.
> - **Non-Arbitrary Operations**: Standard C implementation-defined or undefined behaviors are assigned explicit, deterministic definitions (e.g., signed integer addition traps deterministically, bitwise shifts are modulo-masked, and floating-point operations strictly adhere to IEEE-754 rules).

> [!CAUTION]
> **GOAL 3: ZERO `unsafe` BLOCKS IN RUST COMPILER SOURCE**
> - **Strict Rust Safety**: All Rust source files in both workspace crates (`stricc` and `runtime`) **MUST** enforce `#![deny(unsafe_code)]` at the crate root (`main.rs`, `lib.rs`).
> - **Zero `unsafe` Blocks**: No `unsafe` blocks are permitted anywhere in the compiler codebase. All memory management, tokenization, AST manipulation, and runtime metadata tracking must be implemented using 100% safe Rust abstractions (`std::collections::HashMap`, `Arc`, `Rc`, `RefCell`, standard vector indexing, safe references, standard library file and thread primitives).

---

## 2. Supported C Subset & Formal AST Definitions

To ensure an LLM can generate a working parser and type checker deterministically without getting overwhelmed by full C23 grammar complexity, `stricc` targets a core safe C subset with the following formal AST structure:

### 2.1 Supported Syntax & Language Features
- **Primitive Types**: `int` (32-bit signed), `float` (32-bit single precision), `double` (64-bit double precision), `char` (8-bit byte), `bool` (`_Bool`), `void`.
- **Compound Types**: Single and multi-level pointers (`int*`, `char**`), fixed-size arrays (`int arr[10]`), and named `struct` definitions (`struct Point { int x; int y; }`).
- **Storage & Qualifiers**: `const`, `nullptr`, `auto` (type inference), `constexpr`.
- **Control Flow**: `if` / `else`, `while`, `for`, `return`, `break`, `continue`.
- **Expressions**: Binary arithmetic (`+`, `-`, `*`, `/`, `%`), bitwise (`&`, `|`, `^`, `<<`, `>>`), relational (`==`, `!=`, `<`, `>`, `<=`, `>=`), assignment (`=`), array indexing (`arr[i]`), struct member access (`s.field`, `ptr->field`), address-of (`&x`), dereference (`*p`), function calls (`f(x)`).

### 2.2 Concrete Rust AST Data Models (`ast.rs`)

```rust
#![deny(unsafe_code)]

#[derive(Debug, Clone, PartialEq)]
pub enum Type {
    Int,
    Float,
    Double,
    Char,
    Bool,
    Void,
    Pointer(Box<Type>),
    Array(Box<Type>, usize),
    Struct(String),
}

#[derive(Debug, Clone, PartialEq)]
pub enum BinaryOp {
    Add, Sub, Mul, Div, Mod,
    BitAnd, BitOr, BitXor, Shl, Shr,
    Eq, Ne, Lt, Gt, Le, Ge,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Expr {
    IntLiteral(i32),
    FloatLiteral(f32),
    StringLiteral(String),
    BoolLiteral(bool),
    NullPtr,
    Variable(String),
    Binary(BinaryOp, Box<Expr>, Box<Expr>),
    Assign(String, Box<Expr>),
    ArrayAccess(Box<Expr>, Box<Expr>),
    MemberAccess(Box<Expr>, String, bool), // bool is_arrow
    AddressOf(Box<Expr>),
    Dereference(Box<Expr>),
    Call(String, Vec<Expr>),
}

#[derive(Debug, Clone, PartialEq)]
pub enum Stmt {
    VarDecl { name: String, ty: Type, init: Option<Expr>, is_const: bool },
    Expr(Expr),
    Block(Vec<Stmt>),
    If { cond: Expr, then_branch: Box<Stmt>, else_branch: Option<Box<Stmt>> },
    While { cond: Expr, body: Box<Stmt> },
    For { init: Option<Box<Stmt>>, cond: Option<Expr>, step: Option<Expr>, body: Box<Stmt> },
    Return(Option<Expr>),
}

#[derive(Debug, Clone, PartialEq)]
pub struct Param {
    pub name: String,
    pub ty: Type,
}

#[derive(Debug, Clone, PartialEq)]
pub struct FunctionDecl {
    pub name: String,
    pub params: Vec<Param>,
    pub return_type: Type,
    pub body: Option<Vec<Stmt>>, // None for extern declarations
}

#[derive(Debug, Clone, PartialEq)]
pub struct StructDecl {
    pub name: String,
    pub fields: Vec<(String, Type)>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct TranslationUnit {
    pub structs: Vec<StructDecl>,
    pub functions: Vec<FunctionDecl>,
}
```

---

## 3. Non-Obvious Architectural Features: Explanations & Examples

### 3.1 Decoupled Shadow Metadata Table (Spatial & Temporal Memory Safety)

#### Why Fat Pointers Are Forbidden
Standard "Safe C" dialects often replace 64-bit pointers with 192-bit fat pointers containing `(pointer_address, base_address, bounds_size)`. Fat pointers break C ABI compatibility, making it impossible to pass pointers to precompiled standard C libraries (such as `libc`, `libz.a`, or `sqlite3.o`) or cast structs cleanly.

#### How `stricc` Achieves ABI Compatibility with Decoupled Metadata
`stricc` maintains standard **8-byte physical pointers** (`uintptr_t`). Spatial bounds and temporal allocation keys are tracked in a **Decoupled Shadow Metadata Table** managed by the runtime library `libstricc_rt.a`.

```
Physical Pointer (64-bit address)  ----> [ Standard C Memory (Stack/Heap/Global) ]
                                                   |
Shadow Metadata Table (Address Hash Map) <---------+
└── Maps Address Range -> (base: 0x7ffd..., size: 40 bytes, key: 0x8a3f...)
```

#### Runtime Export Signatures (`runtime/src/lib.rs`)
The runtime library `libstricc_rt.a` exports clean C-compatible FFI helper functions:

```rust
#![deny(unsafe_code)]

#[no_mangle]
pub extern "C" fn stricc_register_allocation(ptr: usize, size: usize) -> u64 {
    // Registers allocation in thread-safe global map using safe Rust Mutex<HashMap<...>>
    // Returns unique 64-bit allocation key
    0
}

#[no_mangle]
pub extern "C" fn stricc_check_access(ptr: usize, size: usize, is_write: i32, file: *const i8, line: i32) {
    // Looks up ptr range in shadow table; aborts via stricc_abort if invalid
}

#[no_mangle]
pub extern "C" fn stricc_malloc(size: usize) -> usize {
    // Safe wrapper calling system allocator and stricc_register_allocation
    0
}

#[no_mangle]
pub extern "C" fn stricc_free(ptr: usize) {
    // Asserts key matches in shadow table, sets key to 0, and frees memory
}

#[no_mangle]
pub extern "C" fn stricc_abort(msg: *const i8, file: *const i8, line: i32) {
    // Captures DWARF stack trace and aborts process with exit code 134
}
```

---

### 3.2 Value Range Propagation (VRP) & Bounds Check Elimination

Generating a shadow table lookup for every single memory access can introduce runtime overhead. `stricc` includes a **Value Range Propagation (VRP)** pass in `typechecker.rs` to statically analyze loop bounds, array sizes, and pointer arithmetic ranges.

#### Concrete Example
```c
void fill_array(void) {
    int arr[10];
    for (int i = 0; i < 10; i++) {
        arr[i] = i; // VRP proves: arr size = 40 bytes, access range [0..36] bytes.
                    // 0 <= i <= 9 is mathematically guaranteed.
                    // Bounds check is completely ELIMINATED at compile-time.
    }
}
```
When VRP proves that `index` is strictly within `[0, array_length - 1]`, `codegen.rs` omits the call to `stricc_check_access`, resulting in zero runtime overhead for proven loops.

---

### 3.3 Command-Line Interface (CLI) Option Pre-Filtering

To allow legacy build systems (`make`, `cmake`, `autotools`) to use `CC=stricc` without failing on unsupported compiler options, `main.rs` performs a **pre-filtering pass** on `std::env::args()` before argument parsing.

#### Pre-Filtering Rules
Flags starting with `-` that match standard host flags non-essential to output generation are silently stripped:
- `-g`, `-g3`, `-gdwarf-4` (Debug flags)
- `-Wall`, `-Wextra`, `-Werror`, `-W...` (Warning flags)
- `-std=c99`, `-std=c11`, `-std=c23` (Standard compliance flags)
- `-fPIC`, `-fstack-protector`, `-f...` (Host code generation flags)

#### Pre-Filtering Example
```bash
# Command executed by legacy Makefile:
stricc -O2 -g -Wall -Wextra -std=c11 -I./include -c -o src/util.o src/util.c

# Pre-filtered CLI vector passed to internal driver parser:
stricc -O2 -I./include -c -o src/util.o src/util.c
```

---

### 3.4 Arithmetic Overflow & Division Trap Guards

Standard C specifies signed integer overflow and division by zero as Undefined Behavior. `stricc` forces deterministic aborts by emitting LLVM overflow intrinsics and explicit divisor guard branches.

#### Signed Integer Addition (`x + y`)
`codegen.rs` lowers binary addition to `@llvm.sadd.with.overflow.i32(i32 x, i32 y)`:
```llvm
%res_struct = call { i32, i1 } @llvm.sadd.with.overflow.i32(i32 %x, i32 %y)
%val = extractvalue { i32, i1 } %res_struct, 0
%overflow = extractvalue { i32, i1 } %res_struct, 1
br i1 %overflow, label %overflow_trap, label %continue

overflow_trap:
  call void @stricc_abort(i8* getelementptr("Integer overflow in addition"), i8* filename, i32 line)
  unreachable

continue:
  ; proceed with %val
```

---

## 4. Undefined Behavior (UB) Mitigation Specifications & Examples

| UB Category | Code Example | `stricc` Behavior | Error / Abort Mode |
| :--- | :--- | :--- | :--- |
| **Spatial Bounds (OOB)** | `int a[5]; a[10] = 1;` | Safe Runtime Abort | Runtime Abort with DWARF Backtrace |
| **Use-After-Free (UAF)** | `int *p = malloc(4); free(p); *p = 1;` | Safe Runtime Abort | Runtime Abort with DWARF Backtrace |
| **Double Free** | `int *p = malloc(4); free(p); free(p);` | Safe Runtime Abort | Runtime Abort with DWARF Backtrace |
| **Division by Zero** | `int z = x / 0;` | Safe Runtime Abort | Runtime Abort with DWARF Backtrace |
| **`INT_MIN / -1` Overflow** | `int z = INT_MIN / -1;` | Safe Runtime Abort | Runtime Abort with DWARF Backtrace |
| **Return Stack Address** | `int* f() { int x = 5; return &x; }` | Compile-Time Rejection | Compiler Semantic Error (`error: returning address of local stack variable 'x'`) |
| **Reaching End of Non-Void** | `int f() { int x = 5; }` | Compile-Time Rejection | Compiler Semantic Error (`error: control reaches end of non-void function`) |
| **Uninitialized Read** | `int x; printf("%d", x);` | Auto-Initialized to `0` | Zero-initialized scalar / pointer (`nullptr`) |
| **Out-of-Bounds Shift** | `int y = x << 35;` | Modulo Masked Shift | Masked count: `35 & 31 = 3`, emits `x << 3` |
| **Strict Aliasing** | `float f = 1.0; int *p = (int*)&f;` | Defined Bit Representation | Safe bit-reload (`-fno-strict-aliasing`) |

---

## 5. System Architecture & Crate Layout

```
stricc/
├── stricc/                     # Compiler Driver & Code Generator Crate
│   ├── src/main.rs             # CLI entrypoint, argument pre-filtering (#![deny(unsafe_code)])
│   ├── src/driver.rs           # Workflow manager (Preprocess -> Lex -> Parse -> Typecheck -> Codegen -> Link)
│   ├── src/lexer.rs            # Tokenizer for preprocessed C source code
│   ├── src/parser.rs           # Hand-written recursive descent parser for C23 AST
│   ├── src/ast.rs              # AST nodes (Statements, Expressions, Types, Declarations)
│   ├── src/typechecker.rs      # Semantic analyzer, type verification, VRP bounds elimination, lifetime checks
│   └── src/codegen.rs          # LLVM IR code generator using inkwell crate
└── runtime/                    # Runtime Support Library Crate (libstricc_rt.a)
    └── src/lib.rs              # Safe memory allocation wrappers, shadow table, DWARF backtrace (#![deny(unsafe_code)])
```

---

## 6. Recommended User Story Roadmap (Incremental Phasing)

To allow autonomous agents (like `noctifab`) to construct the compiler incrementally without context exhaustion, the work is divided into 5 clear milestones:

1. **US-001: CLI Driver, Flag Pre-filtering & System Preprocessor Delegation**
   - Implement `main.rs` pre-filtering and `driver.rs` delegating `gcc -E` / `clang -E`.
2. **US-002: Lexer & Parser for Core C AST**
   - Tokenizer (`lexer.rs`), AST structures (`ast.rs`), and recursive descent parser (`parser.rs`) supporting primitive types, expressions, control flow, and function declarations.
3. **US-003: Typechecker, Lifetime Analysis & Value Range Propagation (VRP)**
   - Type verification (`typechecker.rs`), stack escape analysis (rejecting return of local variable address), and VRP loop bounds range analysis.
4. **US-004: LLVM Codegen & Safety Check Instrumentation**
   - LLVM IR generator (`codegen.rs`) using `inkwell` in safe Rust, emitting integer overflow intrinsics, division zero guards, and shadow bounds check calls.
5. **US-005: Runtime Support Library (`libstricc_rt.a`) & Differential Test Suite**
   - Implement safe shadow table allocator wrapper in `runtime/src/lib.rs` and verify execution against GCC C Torture Suite (`make test-gcc`).

---

## 7. Verification & Test Suite Specifications

1. **GCC C Torture Test Suite**: Over 1,500 test cases (`gcc.c-torture/execute`). `stricc` must compile and execute each test case, matching `gcc` output exactly (`make test-gcc`).
2. **LLVM / Clang Test Suite**: Conformance suite verifying C IR generation, struct alignment, and calling conventions (`make test-llvm`).
3. **Real-World Application Suite**: Compiles and verifies real C codebases: SQLite shell, Doom engine, Lua interpreter, and MiniLisp (`make test-build-apps`).

### 7.1 Mandatory Verification Commands
```bash
# 1. Run all unit and integration tests across workspace
cargo test --workspace

# 2. Run formatting and clippy static checks
cargo fmt --check
cargo clippy -- -D warnings

# 3. Run GCC & Clang differential test suites
make test-gcc
make test-llvm
make test-build-apps
```

---

## 8. Product Manager Definition of Done (DoD) Mandate

1. **Absolute Safety & Determinism**: Zero Undefined Behavior (UB). Memory violations and overflow errors must trigger either a compile-time rejection or a deterministic runtime abort with a symbolicated DWARF backtrace.
2. **Strict No-Unsafe Policy**: Every `.rs` file MUST contain `#![deny(unsafe_code)]`. Zero `unsafe` blocks allowed.
3. **Zero Compiler Warnings**: Clean build under `cargo clippy -- -D warnings` and `cargo fmt --check`.
4. **Differential Test Pass**: 100% pass rate on GCC C Torture Suite execution tests (`make test-gcc`) and LLVM / Clang test targets (`make test-llvm`).


### 3.3. Architectural Guidelines
* **SOLID Principles:** Clear separation between lexing, AST parsing, LLVM codegen, and CLI driver stages.
* **Dependency Injection:** Inject code generators, target triples, and diagnostics printers as trait objects.
