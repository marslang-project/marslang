pub mod ast;
pub mod eval;
pub mod lexer;
pub mod parser;

pub const VERSION: &str = concat!("rs-", env!("CARGO_PKG_VERSION"));

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
        let src = "func f(int a) => a;";
        let js = compile_source_to_js(src).expect("compile failed");
        assert!(js.contains("return a;"), "{js}");
        assert!(!js.contains("return __mars.a;"), "{js}");
    }

    #[test]
    fn bare_assignment_is_treated_as_declaration() {
        let src = "func m{
    x = 1;
    c = Circle(5);
}";
        let js = compile_source_to_js(src).expect("compile failed");
        assert!(js.contains("let x = 1;"), "{js}");
        assert!(js.contains("let c = Circle(5);"), "{js}");
    }

    #[test]
    fn member_assignment_stays_assignment() {
        let src = "func m{
    me.r = 1;
}";
        let js = compile_source_to_js(src).expect("compile failed");
        assert!(js.contains("this.r = 1;"), "{js}");
        assert!(!js.contains("let me.r"), "{js}");
    }
    #[test]
    fn parses_if_and_repeat_blocks() {
        let src =
            "func m{\n    if (true) {\n        out(1);\n    }\n    repeat 2 {\n        out(2);\n    }\n}";
        let js = compile_source_to_js(src).expect("compile failed");
        assert!(js.contains("if (true) {"), "{js}");
        assert!(js.contains("for (let __i = 0; __i < (2); __i++) {"), "{js}");
        assert!(!js.contains("if (true) {;"), "{js}");
    }

    #[test]
    fn parses_nested_call_arguments_with_balanced_commas() {
        let src = "func m{\n    out(a(1,2), 3);\n}";
        let js = compile_source_to_js(src).expect("compile failed");
        assert!(js.contains("__mars.out(__mars.a(1, 2), 3);"), "{js}");
    }
}
