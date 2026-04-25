pub mod ast;
pub mod eval;
pub mod lexer;
pub mod parser;

pub const VERSION: &str = "rs-0.1";

pub fn compile_source_to_js(source: &str) -> Result<String, String> {
    let program = parser::parse_program(source)?;
    Ok(eval::compile_to_js(&program))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn comparison_expression_stmt_is_not_assignment() {
        let src = "func m{\n out(1 == 1);\n}";
        let js = compile_source_to_js(src).expect("compile failed");
        assert!(js.contains("__mars.out(1 == 1);"), "{js}");
        assert!(!js.contains("out(1 = = 1)"), "{js}");
    }

    #[test]
    fn family_call_uses_new() {
        let src = "family Circle{\n    func init(){\n    }\n}\nfunc m{\n    c = Circle(5);\n}";
        let js = compile_source_to_js(src).expect("compile failed");
        assert!(js.contains("new Circle(5)"), "{js}");
    }

    #[test]
    fn builtin_names_can_be_shadowed() {
        let src = "func f(int a;) => a;";
        let js = compile_source_to_js(src).expect("compile failed");
        assert!(js.contains("return a;"), "{js}");
        assert!(!js.contains("return __mars.a;"), "{js}");
    }
}
