pub mod ast;
pub mod codegen;
pub mod driver;
pub mod error;
pub mod lexer;
pub mod parser;
pub mod symbol_table;
pub mod typechecker;

pub const RT_LIB_BYTES: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/libstricc_rt.a"));

pub fn get_runtime_lib_tempfile() -> Result<tempfile::NamedTempFile, std::io::Error> {
    use std::io::Write;
    let mut temp = tempfile::Builder::new()
        .prefix("libstricc_rt")
        .suffix(".a")
        .tempfile()?;
    temp.write_all(RT_LIB_BYTES)?;
    Ok(temp)
}
