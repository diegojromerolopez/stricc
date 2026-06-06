use crate::parser::Parser;
use crate::typechecker::Typechecker;
use crate::codegen::Codegen;
use inkwell::context::Context;
use std::process::Command;
use std::fs;
use std::path::{Path, PathBuf};

pub struct DriverOptions {
    pub input_file: String,
    pub output_file: Option<String>,
    pub compile_only: bool, // -c
    pub assemble_only: bool, // -S
    pub emit_llvm: bool, // -emit-llvm
    pub preprocess_only: bool, // -E
    pub optimization_level: u32, // -O0, -O1, -O2, -O3
    pub include_paths: Vec<String>, // -I
    pub macros: Vec<String>, // -D
}

pub struct Driver {
    options: DriverOptions,
}

impl Driver {
    pub fn new(options: DriverOptions) -> Self {
        Self { options }
    }

    pub fn run(&self) -> Result<(), String> {
        let input_path = Path::new(&self.options.input_file);
        if !input_path.exists() {
            return Err(format!("Input file '{}' does not exist", self.options.input_file));
        }

        // 1. Preprocess using host Clang
        let mut preprocessed_temp = tempfile::Builder::new()
            .suffix(".i")
            .tempfile()
            .map_err(|e| format!("Failed to create temporary file: {}", e))?;
        
        let preprocessed_path = preprocessed_temp.path().to_str().unwrap().to_string();

        let mut cmd = Command::new("clang");
        cmd.arg("-E");
        
        for inc in &self.options.include_paths {
            cmd.arg(format!("-I{}", inc));
        }
        for mac in &self.options.macros {
            cmd.arg(format!("-D{}", mac));
        }
        
        cmd.arg(&self.options.input_file);
        cmd.arg("-o").arg(&preprocessed_path);

        let status = cmd.status().map_err(|e| format!("Failed to execute preprocessor: {}", e))?;
        if !status.success() {
            return Err("Preprocessing failed".to_string());
        }

        if self.options.preprocess_only {
            let content = fs::read_to_string(&preprocessed_path)
                .map_err(|e| format!("Failed to read preprocessed file: {}", e))?;
            println!("{}", content);
            return Ok(());
        }

        // 2. Read preprocessed source code
        let source_code = fs::read_to_string(&preprocessed_path)
            .map_err(|e| format!("Failed to read preprocessed source file: {}", e))?;

        // 3. Parse tokens
        let mut parser = Parser::new(&source_code, &self.options.input_file);
        let mut program = parser.parse_program();

        // Print parser errors if any
        if !parser.errors.is_empty() {
            for err in &parser.errors {
                err.print(&source_code);
            }
            return Err("Parsing failed due to syntax errors".to_string());
        }

        // 4. Typecheck & Semantic Analysis
        let mut typechecker = Typechecker::new(&self.options.input_file);
        if let Err(errors) = typechecker.check_program(&mut program) {
            for err in &errors {
                err.print(&source_code);
            }
            return Err("Typechecking failed".to_string());
        }

        // 5. Code Generation
        let context = Context::create();
        let module = context.create_module(&self.options.input_file);
        let builder = context.create_builder();

        let mut codegen = Codegen::new(&context, &module, &builder, &self.options.input_file);
        codegen.gen_program(&program);

        // 6. Write output
        let ir_str = module.print_to_string().to_string();
        let mut ll_temp = tempfile::Builder::new()
            .suffix(".ll")
            .tempfile()
            .map_err(|e| format!("Failed to create temporary LLVM IR file: {}", e))?;
        
        let ll_path = ll_temp.path().to_str().unwrap().to_string();
        fs::write(&ll_path, &ir_str).map_err(|e| format!("Failed to write LLVM IR: {}", e))?;

        let output_name = self.options.output_file.clone().unwrap_or_else(|| {
            if self.options.compile_only {
                input_path.with_extension("o").to_str().unwrap().to_string()
            } else if self.options.assemble_only {
                input_path.with_extension("s").to_str().unwrap().to_string()
            } else if self.options.emit_llvm {
                input_path.with_extension("ll").to_str().unwrap().to_string()
            } else {
                "a.out".to_string()
            }
        });

        if self.options.emit_llvm {
            fs::copy(&ll_path, &output_name)
                .map_err(|e| format!("Failed to write LLVM IR output: {}", e))?;
            return Ok(());
        }

        if self.options.assemble_only {
            // Compile LLVM IR to Assembly
            let mut cmd = Command::new("clang");
            cmd.arg("-S")
                .arg(format!("-O{}", self.options.optimization_level))
                .arg(&ll_path)
                .arg("-o")
                .arg(&output_name);
            let status = cmd.status().map_err(|e| format!("Failed to run compiler: {}", e))?;
            if !status.success() {
                return Err("Assembly generation failed".to_string());
            }
            return Ok(());
        }

        if self.options.compile_only {
            // Compile LLVM IR to Object file
            let mut cmd = Command::new("clang");
            cmd.arg("-c")
                .arg(format!("-O{}", self.options.optimization_level))
                .arg(&ll_path)
                .arg("-o")
                .arg(&output_name);
            let status = cmd.status().map_err(|e| format!("Failed to run compiler: {}", e))?;
            if !status.success() {
                return Err("Object code generation failed".to_string());
            }
            return Ok(());
        }

        // Link executable with runtime static library
        let mut cmd = Command::new("clang");
        cmd.arg(format!("-O{}", self.options.optimization_level))
            .arg(&ll_path);

        let mut rt_path = None;

        // 1. Try finding it in the same directory as the compiler executable
        if let Ok(exe_path) = std::env::current_exe() {
            if let Some(exe_dir) = exe_path.parent() {
                let candidate = exe_dir.join("libstricc_rt.a");
                if candidate.exists() {
                    rt_path = Some(candidate.to_str().unwrap().to_string());
                }
            }
        }

        // 2. Try STRICC_RT_PATH environment variable
        if rt_path.is_none() {
            if let Ok(env_path) = std::env::var("STRICC_RT_PATH") {
                if Path::new(&env_path).exists() {
                    rt_path = Some(env_path);
                }
            }
        }

        // 3. Fallback to workspace root directory (dynamically detected or default)
        if rt_path.is_none() {
            let workspace_root = if let Ok(dir) = std::env::var("CARGO_MANIFEST_DIR") {
                PathBuf::from(dir).parent().unwrap().to_path_buf()
            } else {
                PathBuf::from("/Users/diegoj/repos/stricc")
            };
            let possible_rt_paths = vec![
                workspace_root.join("target/debug/libstricc_rt.a"),
                workspace_root.join("target/release/libstricc_rt.a"),
            ];
            for path in &possible_rt_paths {
                if path.exists() {
                    rt_path = Some(path.to_str().unwrap().to_string());
                    break;
                }
            }
        }

        let rt_lib = match rt_path {
            Some(p) => p,
            None => {
                // If not found, guess debug path in default workspace root
                "/Users/diegoj/repos/stricc/target/debug/libstricc_rt.a".to_string()
            }
        };

        cmd.arg(rt_lib);
        cmd.arg("-o").arg(&output_name);

        let status = cmd.status().map_err(|e| format!("Failed to run linker: {}", e))?;
        if !status.success() {
            return Err("Linking failed".to_string());
        }

        Ok(())
    }
}
