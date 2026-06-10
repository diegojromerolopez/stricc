use crate::codegen::Codegen;
use crate::parser::Parser;
use crate::typechecker::Typechecker;
use inkwell::context::Context;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

// ─── CompilerError ────────────────────────────────────────────────────────────

/// Structured error type for every failure the compiler pipeline can produce.
///
/// Replaces the raw `String` error that `run()` previously returned so that
/// call sites can pattern-match on the failure reason (DIP / OCP).
#[derive(Debug)]
pub enum CompilerError {
    /// An I/O operation failed.
    Io(String),
    /// The source file uses a construct that Safe-C forbids.
    ForbiddenConstruct(String),
    /// One or more lexer / parser errors were detected.
    ParseError,
    /// One or more type-checking errors were detected.
    TypeCheckError,
    /// Code generation produced an error.
    CodegenError(String),
    /// The assembler step failed.
    AssemblyError,
    /// The linker step failed.
    LinkError,
}

impl std::fmt::Display for CompilerError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CompilerError::Io(msg) => write!(f, "{msg}"),
            CompilerError::ForbiddenConstruct(msg) => write!(f, "{msg}"),
            CompilerError::ParseError => write!(f, "Parsing failed due to syntax errors"),
            CompilerError::TypeCheckError => write!(f, "Typechecking failed"),
            CompilerError::CodegenError(msg) => write!(f, "Code generation failed: {msg}"),
            CompilerError::AssemblyError => write!(f, "Assembly generation failed"),
            CompilerError::LinkError => write!(f, "Linking failed"),
        }
    }
}

impl std::error::Error for CompilerError {}

// ─── CompilerPipeline trait ───────────────────────────────────────────────────

/// Abstraction over the full compile-then-emit flow.
///
/// This makes it possible to inject alternative pipelines (e.g. a no-op for
/// benchmarking, or a pipeline that emits diagnostics to a buffer in tests).
pub trait CompilerPipeline {
    /// Execute the pipeline and either produce the requested output or return
    /// a structured [`CompilerError`].
    fn run(&self) -> Result<(), CompilerError>;
}

// ─── DriverOptions ────────────────────────────────────────────────────────────

/// Configuration options passed to the compiler driver.
pub struct DriverOptions {
    pub input_file: String,
    pub output_file: Option<String>,
    pub compile_only: bool,         // -c
    pub assemble_only: bool,        // -S
    pub emit_llvm: bool,            // --emit-llvm
    pub preprocess_only: bool,      // -E
    pub optimization_level: u32,    // -O0 … -O3
    pub include_paths: Vec<String>, // -I
    pub macros: Vec<String>,        // -D
}

// ─── Driver ───────────────────────────────────────────────────────────────────

/// The top-level compiler driver.
///
/// `Driver` is a thin **orchestrator**: it delegates each pipeline stage to a
/// focused helper (forbidden-construct check, preprocessor, parser, type-
/// checker, code generator, output writer, linker).  No stage logic lives
/// directly in `run()`.
pub struct Driver {
    options: DriverOptions,
}

impl Driver {
    pub fn new(options: DriverOptions) -> Self {
        Self { options }
    }
}

impl CompilerPipeline for Driver {
    fn run(&self) -> Result<(), CompilerError> {
        let input_path = Path::new(&self.options.input_file);

        // ── Stage 0: read raw source ──────────────────────────────────────────
        let raw_content = read_source_file(input_path, &self.options.input_file)?;

        // ── Stage 1: forbidden-construct check ────────────────────────────────
        ForbiddenConstructChecker::check(&raw_content)?;

        // ── Stage 2: preprocess via clang -E ─────────────────────────────────
        let preprocessed_path = Preprocessor::run(
            &self.options.input_file,
            &self.options.include_paths,
            &self.options.macros,
        )?;

        if self.options.preprocess_only {
            let content = fs::read_to_string(&preprocessed_path)
                .map_err(|e| CompilerError::Io(format!("Failed to read preprocessed file: {e}")))?;
            println!("{content}");
            return Ok(());
        }

        let source_code = fs::read_to_string(&preprocessed_path)
            .map_err(|e| CompilerError::Io(format!("Failed to read preprocessed source: {e}")))?;

        // ── Stage 3: parse ────────────────────────────────────────────────────
        let mut program = parse_source(&source_code, &self.options.input_file)?;

        // ── Stage 4: type-check ───────────────────────────────────────────────
        typecheck_program(&mut program, &source_code, &self.options.input_file)?;

        // ── Stage 5: code generation ──────────────────────────────────────────
        let ir_str = generate_llvm_ir(&program, &self.options.input_file)?;

        // ── Stage 6: write output ─────────────────────────────────────────────
        let ll_path = write_llvm_ir_to_temp(&ir_str)?;

        let output_name = resolve_output_name(input_path, &self.options);

        OutputWriter::emit(&ll_path, &output_name, &self.options)
    }
}

// ─── Stage helpers ────────────────────────────────────────────────────────────

/// Read the raw (un-preprocessed) source file from disk.
fn read_source_file(path: &Path, filename: &str) -> Result<String, CompilerError> {
    if !path.exists() {
        return Err(CompilerError::Io(format!(
            "Input file '{filename}' does not exist"
        )));
    }
    fs::read_to_string(path)
        .map_err(|e| CompilerError::Io(format!("Failed to read input file: {e}")))
}

/// Parse preprocessed source and return the program AST.
fn parse_source(source_code: &str, filename: &str) -> Result<crate::ast::Program, CompilerError> {
    let mut parser = Parser::new(source_code, filename);
    let program = parser.parse_program();
    if !parser.errors.is_empty() {
        for err in &parser.errors {
            err.print(source_code);
        }
        return Err(CompilerError::ParseError);
    }
    Ok(program)
}

/// Run the type-checker / semantic analyser over the AST.
fn typecheck_program(
    program: &mut crate::ast::Program,
    source_code: &str,
    filename: &str,
) -> Result<(), CompilerError> {
    let mut typechecker = Typechecker::new(filename);
    if let Err(errors) = typechecker.check_program(program) {
        for err in &errors {
            err.print(source_code);
        }
        return Err(CompilerError::TypeCheckError);
    }
    Ok(())
}

/// Lower the AST to LLVM IR and return it as a string.
fn generate_llvm_ir(
    program: &crate::ast::Program,
    filename: &str,
) -> Result<String, CompilerError> {
    let context = Context::create();
    let module = context.create_module(filename);
    let builder = context.create_builder();

    let mut codegen = Codegen::new(&context, &module, &builder, filename);
    codegen.gen_program(program);

    Ok(module.print_to_string().to_string())
}

/// Write LLVM IR to a temporary `.ll` file and return its path.
fn write_llvm_ir_to_temp(ir_str: &str) -> Result<String, CompilerError> {
    let ll_temp = tempfile::Builder::new()
        .suffix(".ll")
        .tempfile()
        .map_err(|e| CompilerError::Io(format!("Failed to create temporary LLVM IR file: {e}")))?;
    let ll_path = ll_temp.path().to_str().unwrap().to_string();
    // Keep the NamedTempFile alive for the duration of this function so the OS
    // does not delete it.  We persist (convert to path) so it survives.
    ll_temp
        .keep()
        .map_err(|e| CompilerError::Io(format!("Failed to persist temp file: {e}")))?;
    fs::write(&ll_path, ir_str)
        .map_err(|e| CompilerError::Io(format!("Failed to write LLVM IR: {e}")))?;
    Ok(ll_path)
}

/// Compute the final output path from options.
fn resolve_output_name(input_path: &Path, options: &DriverOptions) -> String {
    options.output_file.clone().unwrap_or_else(|| {
        if options.compile_only {
            input_path.with_extension("o").to_str().unwrap().to_string()
        } else if options.assemble_only {
            input_path.with_extension("s").to_str().unwrap().to_string()
        } else if options.emit_llvm {
            input_path
                .with_extension("ll")
                .to_str()
                .unwrap()
                .to_string()
        } else {
            "a.out".to_string()
        }
    })
}

// ─── ForbiddenConstructChecker ────────────────────────────────────────────────

/// Checks source text for constructs that are unconditionally forbidden in
/// Safe-C mode **before** preprocessing.
///
/// **Single responsibility**: this struct knows only about forbidden patterns.
/// It does not read files, parse tokens, or produce LLVM IR.
struct ForbiddenConstructChecker;

impl ForbiddenConstructChecker {
    fn check(content: &str) -> Result<(), CompilerError> {
        check_keyword_redefinitions(content).map_err(CompilerError::ForbiddenConstruct)?;

        if content.contains("#include <setjmp.h>") {
            return Err(CompilerError::ForbiddenConstruct(
                "Header <setjmp.h> is forbidden in Safe C mode".to_string(),
            ));
        }
        if is_identifier_present(content, "setjmp") || is_identifier_present(content, "longjmp") {
            return Err(CompilerError::ForbiddenConstruct(
                "setjmp/longjmp are forbidden in Safe C mode (use structured control flow)"
                    .to_string(),
            ));
        }
        if content.contains("#include <threads.h>") || content.contains("#include <pthread.h>") {
            return Err(CompilerError::ForbiddenConstruct(
                "Multi-threading headers are forbidden in Safe C mode".to_string(),
            ));
        }
        if content.contains("__asm__") || content.contains("asm(") {
            return Err(CompilerError::ForbiddenConstruct(
                "Inline assembly is forbidden in Safe C mode".to_string(),
            ));
        }
        if is_identifier_present(content, "asm") {
            return Err(CompilerError::ForbiddenConstruct(
                "Inline assembly is forbidden in Safe C mode".to_string(),
            ));
        }
        Ok(())
    }
}

// ─── Preprocessor ─────────────────────────────────────────────────────────────

/// Invokes `clang -E` and returns the path to the preprocessed output file.
///
/// **Single responsibility**: knows only how to run the C preprocessor.
struct Preprocessor;

impl Preprocessor {
    fn run(
        input_file: &str,
        include_paths: &[String],
        macros: &[String],
    ) -> Result<String, CompilerError> {
        let preprocessed_temp = tempfile::Builder::new()
            .suffix(".i")
            .tempfile()
            .map_err(|e| CompilerError::Io(format!("Failed to create temporary file: {e}")))?;
        let preprocessed_path = preprocessed_temp.path().to_str().unwrap().to_string();
        preprocessed_temp
            .keep()
            .map_err(|e| CompilerError::Io(format!("Failed to persist temp file: {e}")))?;

        let mut cmd = Command::new("clang");
        cmd.arg("-E");
        for inc in include_paths {
            cmd.arg(format!("-I{inc}"));
        }
        for mac in macros {
            cmd.arg(format!("-D{mac}"));
        }
        cmd.arg(input_file).arg("-o").arg(&preprocessed_path);

        let status = cmd
            .status()
            .map_err(|e| CompilerError::Io(format!("Failed to execute preprocessor: {e}")))?;
        if !status.success() {
            return Err(CompilerError::Io("Preprocessing failed".to_string()));
        }
        Ok(preprocessed_path)
    }
}

// ─── OutputWriter ─────────────────────────────────────────────────────────────

/// Converts the LLVM IR temp file into the final requested output format by
/// invoking `clang` (for `.o`, `.s`) or the `Linker` (for executables).
///
/// **Single responsibility**: knows only about the output-format decision tree.
struct OutputWriter;

impl OutputWriter {
    fn emit(
        ll_path: &str,
        output_name: &str,
        options: &DriverOptions,
    ) -> Result<(), CompilerError> {
        if options.emit_llvm {
            fs::copy(ll_path, output_name)
                .map_err(|e| CompilerError::Io(format!("Failed to write LLVM IR output: {e}")))?;
            return Ok(());
        }

        if options.assemble_only {
            let status = Command::new("clang")
                .arg("-S")
                .arg(format!("-O{}", options.optimization_level))
                .arg(ll_path)
                .arg("-o")
                .arg(output_name)
                .status()
                .map_err(|e| CompilerError::Io(format!("Failed to run compiler: {e}")))?;
            if !status.success() {
                return Err(CompilerError::AssemblyError);
            }
            return Ok(());
        }

        if options.compile_only {
            let status = Command::new("clang")
                .arg("-c")
                .arg(format!("-O{}", options.optimization_level))
                .arg(ll_path)
                .arg("-o")
                .arg(output_name)
                .status()
                .map_err(|e| CompilerError::Io(format!("Failed to run compiler: {e}")))?;
            if !status.success() {
                return Err(CompilerError::AssemblyError);
            }
            return Ok(());
        }

        // Full link
        Linker::link(ll_path, output_name, options.optimization_level)
    }
}

// ─── Linker ───────────────────────────────────────────────────────────────────

/// Resolves the runtime static library path and invokes the system linker.
///
/// **Single responsibility**: knows only about finding `libstricc_rt.a` and
/// invoking `clang` for final linking.
struct Linker;

impl Linker {
    fn link(ll_path: &str, output_name: &str, opt_level: u32) -> Result<(), CompilerError> {
        let rt_lib = Self::resolve_runtime_lib();

        let status = Command::new("clang")
            .arg(format!("-O{opt_level}"))
            .arg(ll_path)
            .arg(&rt_lib)
            .arg("-o")
            .arg(output_name)
            .status()
            .map_err(|e| CompilerError::Io(format!("Failed to run linker: {e}")))?;

        if !status.success() {
            return Err(CompilerError::LinkError);
        }
        Ok(())
    }

    /// Search for `libstricc_rt.a` in the following order:
    ///
    /// 1. Same directory as the running compiler executable.
    /// 2. `STRICC_RT_PATH` environment variable.
    /// 3. `target/{debug,release}/` relative to the Cargo workspace root
    ///    (detected via `CARGO_MANIFEST_DIR` or a hard-coded fallback).
    fn resolve_runtime_lib() -> String {
        // 1. Alongside the compiler binary
        if let Ok(exe_path) = std::env::current_exe() {
            if let Some(exe_dir) = exe_path.parent() {
                let candidate = exe_dir.join("libstricc_rt.a");
                if candidate.exists() {
                    return candidate.to_str().unwrap().to_string();
                }
            }
        }

        // 2. Environment variable
        if let Ok(env_path) = std::env::var("STRICC_RT_PATH") {
            if Path::new(&env_path).exists() {
                return env_path;
            }
        }

        // 3. Workspace root heuristic
        let workspace_root = if let Ok(dir) = std::env::var("CARGO_MANIFEST_DIR") {
            PathBuf::from(dir)
                .parent()
                .expect("CARGO_MANIFEST_DIR has no parent")
                .to_path_buf()
        } else {
            PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .parent()
                .expect("CARGO_MANIFEST_DIR has no parent")
                .to_path_buf()
        };

        for sub in &[
            "target/debug/libstricc_rt.a",
            "target/release/libstricc_rt.a",
        ] {
            let candidate = workspace_root.join(sub);
            if candidate.exists() {
                return candidate.to_str().unwrap().to_string();
            }
        }

        // Last resort: compile-time known path (replaces hardcoded runtime string)
        workspace_root
            .join("target/debug/libstricc_rt.a")
            .to_str()
            .unwrap()
            .to_string()
    }
}

// ─── Low-level text utilities ─────────────────────────────────────────────────
//
// These are pure text-processing functions used by `ForbiddenConstructChecker`.
// They are private to this module.

fn strip_line_continuations(content: &str) -> String {
    content.replace("\\\r\n", "").replace("\\\n", "")
}

fn strip_comments(content: &str) -> String {
    let mut result = String::new();
    let mut chars = content.chars().peekable();
    let mut in_line_comment = false;
    let mut in_block_comment = false;

    while let Some(c) = chars.next() {
        if in_line_comment {
            if c == '\n' {
                in_line_comment = false;
                result.push('\n');
            }
        } else if in_block_comment {
            if c == '*' && chars.peek() == Some(&'/') {
                chars.next();
                in_block_comment = false;
                result.push(' ');
            }
        } else if c == '/' && chars.peek() == Some(&'/') {
            chars.next();
            in_line_comment = true;
        } else if c == '/' && chars.peek() == Some(&'*') {
            chars.next();
            in_block_comment = true;
        } else {
            result.push(c);
        }
    }
    result
}

fn check_keyword_redefinitions(content: &str) -> Result<(), String> {
    let spliced = strip_line_continuations(content);
    let stripped = strip_comments(&spliced);

    const KEYWORDS: &[&str] = &[
        "int",
        "char",
        "float",
        "double",
        "short",
        "long",
        "unsigned",
        "signed",
        "void",
        "struct",
        "union",
        "enum",
        "const",
        "auto",
        "nullptr",
        "constexpr",
        "typeof",
        "bool",
        "true",
        "false",
        "if",
        "else",
        "while",
        "for",
        "switch",
        "case",
        "default",
        "break",
        "continue",
        "return",
        "sizeof",
        "alignof",
        "_Atomic",
        "__unsafe",
        "restrict",
    ];

    for line in stripped.lines() {
        let trimmed = line.trim();
        if let Some(rest_after_hash) = trimmed.strip_prefix('#') {
            let rest = rest_after_hash.trim();
            if let Some(rest_define) = rest.strip_prefix("define") {
                let rest_define = rest_define.trim();
                let mut macro_name = String::new();
                for c in rest_define.chars() {
                    if c.is_alphanumeric() || c == '_' {
                        macro_name.push(c);
                    } else {
                        break;
                    }
                }
                if !macro_name.is_empty() && KEYWORDS.contains(&macro_name.as_str()) {
                    return Err(format!(
                        "Redefining keyword '{macro_name}' as a macro is forbidden in Safe C mode"
                    ));
                }
            }
        }
    }
    Ok(())
}

/// Returns `true` if `word` appears in `content` as a standalone identifier.
fn is_identifier_present(content: &str, word: &str) -> bool {
    let bytes = content.as_bytes();
    let word_bytes = word.as_bytes();
    let wlen = word_bytes.len();
    if wlen == 0 {
        return false;
    }
    let clen = bytes.len();
    if clen < wlen {
        return false;
    }
    let mut i = 0;
    while i + wlen <= clen {
        if &bytes[i..i + wlen] == word_bytes {
            let before_ok =
                i == 0 || !(bytes[i - 1] as char).is_alphanumeric() && bytes[i - 1] != b'_';
            let after_ok = (i + wlen) >= clen
                || !(bytes[i + wlen] as char).is_alphanumeric() && bytes[i + wlen] != b'_';
            if before_ok && after_ok {
                return true;
            }
        }
        i += 1;
    }
    false
}
