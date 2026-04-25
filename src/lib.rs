pub mod ast;
pub mod eval;
pub mod lexer;
pub mod parser;

pub const VERSION: &str = "rs-0.1";

pub fn compile_source_to_js(source: &str) -> Result<String, String> {
    let program = parser::parse_program(source)?;
    Ok(eval::compile_to_js(&program))
}
