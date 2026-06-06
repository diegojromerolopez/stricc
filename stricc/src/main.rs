use clap::{Arg, Command};
use stricc::driver::{Driver, DriverOptions};

fn main() {
    let matches = Command::new("stricc")
        .version("0.1.0")
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

    // For simplicity, process the first input file
    let input_file = inputs[0].clone();

    let options = DriverOptions {
        input_file,
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
        eprintln!("stricc: error: {}", err);
        std::process::exit(1);
    }
}
