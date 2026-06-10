use clap::{Arg, Command};
use stricc::driver::{CompilerPipeline, Driver, DriverOptions};

fn main() {
    let matches = Command::new("stricc")
        .version(env!("CARGO_PKG_VERSION"))
        .about("stricc: Safe C Compiler")
        .arg(
            Arg::new("inputs")
                .help("Input C source files")
                .required(true)
                .num_args(1..),
        )
        .arg(
            Arg::new("output")
                .short('o')
                .long("output")
                .help("Output file path")
                .num_args(1),
        )
        .arg(
            Arg::new("compile_only")
                .short('c')
                .help("Compile only; do not link")
                .action(clap::ArgAction::SetTrue),
        )
        .arg(
            Arg::new("assemble_only")
                .short('S')
                .help("Assemble only; compile but do not link/assemble")
                .action(clap::ArgAction::SetTrue),
        )
        .arg(
            Arg::new("emit_llvm")
                .long("emit-llvm")
                .help("Emit LLVM IR instead of object/binary")
                .action(clap::ArgAction::SetTrue),
        )
        .arg(
            Arg::new("preprocess_only")
                .short('E')
                .help("Preprocess only")
                .action(clap::ArgAction::SetTrue),
        )
        .arg(
            Arg::new("optimize")
                .short('O')
                .help("Optimization level (0, 1, 2, 3)")
                .num_args(1)
                .default_value("0"),
        )
        .arg(
            Arg::new("include_paths")
                .short('I')
                .help("Include search paths")
                .action(clap::ArgAction::Append),
        )
        .arg(
            Arg::new("macros")
                .short('D')
                .help("Define macro")
                .action(clap::ArgAction::Append),
        )
        .get_matches();

    let inputs: Vec<String> = matches
        .get_many::<String>("inputs")
        .unwrap()
        .map(|s| s.to_string())
        .collect();

    let output_file = matches.get_one::<String>("output").cloned();
    let compile_only = matches.get_flag("compile_only");
    let assemble_only = matches.get_flag("assemble_only");
    let emit_llvm = matches.get_flag("emit_llvm");
    let preprocess_only = matches.get_flag("preprocess_only");
    let opt_level: u32 = matches
        .get_one::<String>("optimize")
        .unwrap()
        .parse()
        .unwrap_or(0);

    let include_paths: Vec<String> = matches
        .get_many::<String>("include_paths")
        .unwrap_or_default()
        .map(|s| s.to_string())
        .collect();

    let macros: Vec<String> = matches
        .get_many::<String>("macros")
        .unwrap_or_default()
        .map(|s| s.to_string())
        .collect();

    if inputs.len() == 1 {
        // Single input — straightforward pipeline
        let options = DriverOptions {
            input_file: inputs[0].clone(),
            output_file,
            compile_only,
            assemble_only,
            emit_llvm,
            preprocess_only,
            optimization_level: opt_level,
            include_paths,
            macros,
        };
        let driver = Driver::new(options);
        if let Err(err) = driver.run() {
            eprintln!("stricc: error: {err}");
            std::process::exit(1);
        }
    } else {
        // Multiple inputs
        if preprocess_only || emit_llvm || assemble_only {
            eprintln!(
                "stricc: error: cannot specify -E, -S, or --emit-llvm with multiple input files"
            );
            std::process::exit(1);
        }

        if compile_only {
            // Compile each input to its own object file
            for input in &inputs {
                let options = DriverOptions {
                    input_file: input.clone(),
                    output_file: None,
                    compile_only: true,
                    assemble_only: false,
                    emit_llvm: false,
                    preprocess_only: false,
                    optimization_level: opt_level,
                    include_paths: include_paths.clone(),
                    macros: macros.clone(),
                };
                let driver = Driver::new(options);
                if let Err(err) = driver.run() {
                    eprintln!("stricc: error compiling '{input}': {err}");
                    std::process::exit(1);
                }
            }
        } else {
            // Compile each input to a temp object file, then link them all
            let mut obj_files = Vec::new();
            let mut temp_files = Vec::new();

            for input in &inputs {
                let temp_obj = tempfile::Builder::new().suffix(".o").tempfile().unwrap();
                let temp_obj_path = temp_obj.path().to_str().unwrap().to_string();

                let options = DriverOptions {
                    input_file: input.clone(),
                    output_file: Some(temp_obj_path.clone()),
                    compile_only: true,
                    assemble_only: false,
                    emit_llvm: false,
                    preprocess_only: false,
                    optimization_level: opt_level,
                    include_paths: include_paths.clone(),
                    macros: macros.clone(),
                };
                let driver = Driver::new(options);
                if let Err(err) = driver.run() {
                    eprintln!("stricc: error compiling '{input}': {err}");
                    std::process::exit(1);
                }
                obj_files.push(temp_obj_path);
                temp_files.push(temp_obj);
            }

            // Final link step (reuse Linker logic via a direct clang call)
            let output_name = output_file.unwrap_or_else(|| "a.out".to_string());
            let mut cmd = std::process::Command::new("clang");
            cmd.arg(format!("-O{opt_level}"));
            for obj in &obj_files {
                cmd.arg(obj);
            }

            // Runtime lib path is resolved the same way as in Linker::resolve_runtime_lib().
            // We replicate it here to avoid making Linker public — the actual logic is in driver.rs.
            let rt_lib = resolve_rt_lib_path();
            cmd.arg(rt_lib).arg("-o").arg(&output_name);

            let status = cmd
                .status()
                .map_err(|e| format!("Failed to run linker: {e}"))
                .unwrap();
            if !status.success() {
                eprintln!("stricc: error: Linking failed");
                std::process::exit(1);
            }
        }
    }
}

/// Resolve the runtime library path for the multi-file link step.
///
/// Mirrors `Linker::resolve_runtime_lib` in `driver.rs`.
fn resolve_rt_lib_path() -> String {
    if let Ok(exe_path) = std::env::current_exe() {
        if let Some(exe_dir) = exe_path.parent() {
            let candidate = exe_dir.join("libstricc_rt.a");
            if candidate.exists() {
                return candidate.to_str().unwrap().to_string();
            }
        }
    }
    if let Ok(env_path) = std::env::var("STRICC_RT_PATH") {
        if std::path::Path::new(&env_path).exists() {
            return env_path;
        }
    }
    let workspace_root = if let Ok(dir) = std::env::var("CARGO_MANIFEST_DIR") {
        std::path::PathBuf::from(dir)
            .parent()
            .unwrap()
            .to_path_buf()
    } else {
        std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
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
    workspace_root
        .join("target/debug/libstricc_rt.a")
        .to_str()
        .unwrap()
        .to_string()
}
