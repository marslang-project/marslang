pub mod ast;
mod date;
mod expression;
mod gc;
mod interp;
pub mod lexer;
mod package;
pub mod parser;
mod resolve;
mod symbols;
pub mod value;
#[cfg(windows)]
mod windows_zones;

pub use interp::InputSource;
pub use package::user_packages;
pub use value::{ErrorKind, RuntimeError};

pub const VERSION: &str = concat!("rs-", env!("CARGO_PKG_VERSION"));

/// Interpreter threads get a large stack so deep Marslang recursion reaches
/// `interp::MAX_CALL_DEPTH` (a catchable RangeError) instead of overflowing.
const INTERPRETER_STACK: usize = 512 * 1024 * 1024;

/// A parsed and resolved program with the packages it imports, ready to run.
pub struct Compiled {
    program: ast::Program,
    packages: Vec<package::LoadedPackage>,
}

impl std::fmt::Debug for Compiled {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        let names: Vec<&str> = self.packages.iter().map(|p| p.name.as_str()).collect();
        write!(f, "Compiled {{ {} items, packages: {names:?} }}", self.program.items.len())
    }
}

/// Parse and resolve a program, loading the packages it imports. Package files
/// (`takepkg a.b;`) are found relative to the current directory. Syntax errors,
/// unknown names, and fixed-binding reassignment are reported here.
pub fn compile(source: &str) -> Result<Compiled, String> {
    compile_in(source, std::path::Path::new("."))
}

/// Compile a source file; package files are found relative to its directory.
pub fn compile_file(path: &std::path::Path) -> Result<Compiled, String> {
    let source = std::fs::read_to_string(path)
        .map_err(|e| format!("failed to read source file {}: {e}", path.display()))?;
    compile_in(&source, path.parent().unwrap_or(std::path::Path::new(".")))
}

fn compile_in(source: &str, base: &std::path::Path) -> Result<Compiled, String> {
    let mut program = parser::parse_program(source)?;
    let mut loader = package::Loader::new(base);
    loader.load_imports(&mut program, None)?;
    package::apply_decorators(&mut program, &loader.markers())?;
    resolve::resolve(&mut program)?;
    Ok(Compiled { program, packages: loader.packages })
}

/// What a program declares, as JSON: functions, families, and methods with their
/// parameters, source lines, and docstrings, and variables with their scope and
/// type. This is `marslang symbols`, which editors read for hover information.
/// Packages are loaded, from `base`, so decorators are checked as in `compile`.
pub fn symbols(source: &str, base: &std::path::Path) -> Result<String, String> {
    let mut program = parser::parse_program(source)?;
    let mut loader = package::Loader::new(base);
    loader.load_imports(&mut program, None)?;
    package::apply_decorators(&mut program, &loader.markers())?;
    let imports: Vec<(String, &package::LoadedPackage)> = program.items.iter().filter_map(|item| match item {
        ast::Item::Import(import) => {
            let key = import.key.as_deref()?;
            let package = loader.packages.iter().find(|package| package.key == key)?;
            Some((import.alias.clone()?, package))
        }
        _ => None,
    }).collect();
    Ok(symbols::describe(&program, &imports))
}

/// Run a compiled program with the process's stdin/stdout.
pub fn run(compiled: Compiled) -> Result<(), RuntimeError> {
    run_with_args(compiled, String::new(), Vec::new())
}

/// Run as `marslang PROGRAM ARGS...`: `program` is the path of the file as it
/// was given and `args` the words after it, which `std.cli` reads. A program
/// that calls `cli.exit(code)` ends with an error whose `exit` holds the code.
pub fn run_with_args(compiled: Compiled, program: String, args: Vec<String>) -> Result<(), RuntimeError> {
    on_interpreter_thread(move || {
        value::set_command_line(program, args);
        use std::io::{IsTerminal, Write};
        let stdout = std::io::stdout();
        // A terminal sees each `out` immediately; piped output is buffered.
        let interactive = stdout.is_terminal();
        let mut out = std::io::BufWriter::new(stdout.lock());
        let result = interp::Interp::new(&mut out, InputSource::Stdin, interactive).run(&compiled.program, &compiled.packages);
        let flushed = out.flush().map_err(|e| RuntimeError::new(ErrorKind::Error, format!("failed to write output: {e}")));
        result.and(flushed)
    })
}

/// Run a compiled program with the given standard input, capturing its output.
/// Output written before a runtime error is kept.
pub fn run_captured(compiled: Compiled, input: &str) -> (String, Result<(), RuntimeError>) {
    run_captured_with_args(compiled, input, String::new(), Vec::new())
}

/// `run_captured`, with a command line for `std.cli` (see `run_with_args`).
pub fn run_captured_with_args(compiled: Compiled, input: &str, program: String, args: Vec<String>) -> (String, Result<(), RuntimeError>) {
    let input = input.to_string();
    on_interpreter_thread(move || {
        value::set_command_line(program, args);
        let mut out = Vec::new();
        let result = interp::Interp::new(&mut out, InputSource::Text(input), false).run(&compiled.program, &compiled.packages);
        (String::from_utf8_lossy(&out).into_owned(), result)
    })
}

fn on_interpreter_thread<T: Send + 'static>(work: impl FnOnce() -> T + Send + 'static) -> T {
    std::thread::Builder::new()
        .name("marslang".into())
        .stack_size(INTERPRETER_STACK)
        .spawn(work)
        .expect("failed to start interpreter thread")
        .join()
        .unwrap_or_else(|panic| std::panic::resume_unwind(panic))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn output(source: &str) -> String {
        let (out, result) = run_captured(compile(source).expect("compile failed"), "");
        result.expect("runtime error");
        out
    }

    #[test]
    fn comparison_expression_stmt_is_not_assignment() {
        assert_eq!(output("func m{\n out(1 == 1);\n}"), "true\n");
    }

    #[test]
    fn family_call_constructs_instance() {
        let src = "family Circle{\n    func init(int r){\n    me.r = r;\n    }\n}\nfunc m{\n    c = Circle(5);\n    out(c.r);\n}";
        assert_eq!(output(src), "5\n");
    }

    #[test]
    fn builtin_names_can_be_shadowed_by_parameters() {
        assert_eq!(output("func f(int out) => out + 1;\nfunc m{ slout(f(2)); }"), "3");
    }

    #[test]
    fn bare_assignment_is_treated_as_declaration() {
        assert_eq!(output("func m{\n    x = 1;\n    c = 5;\n    out(x + c);\n}"), "6\n");
    }

    #[test]
    fn member_assignment_outside_a_method_needs_me() {
        let (_, result) = run_captured(compile("func m{\n    me.r = 1;\n}").unwrap(), "");
        assert!(result.unwrap_err().message.contains("me"));
    }

    #[test]
    fn parses_if_and_repeat_blocks() {
        let src = "func m{\n    if (true) {\n        out(1);\n    }\n    repeat 2 {\n        out(2);\n    }\n}";
        assert_eq!(output(src), "1\n2\n2\n");
    }

    #[test]
    fn parses_nested_call_arguments_with_balanced_commas() {
        assert_eq!(output("func m{\n    out(arr(1,2), 3);\n}"), "[1, 2] 3\n");
    }

    /// Records writes, marking each flush with `|`.
    struct FlushLog(String);
    impl std::io::Write for FlushLog {
        fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
            self.0.push_str(&String::from_utf8_lossy(bytes));
            Ok(bytes.len())
        }
        fn flush(&mut self) -> std::io::Result<()> { self.0.push('|'); Ok(()) }
    }

    fn flush_log(source: &str, interactive: bool) -> String {
        let compiled = compile(source).unwrap();
        let mut log = FlushLog(String::new());
        interp::Interp::new(&mut log, InputSource::Text("hello".into()), interactive)
            .run(&compiled.program, &compiled.packages).unwrap();
        log.0
    }

    #[test]
    fn prompts_are_flushed_before_reading_input() {
        let program = "func m{ slout(\"name? \"); out(in()); out(\"a\"); line = inln(); }";
        assert_eq!(flush_log(program, false), "name? |hello\na\n|");
        assert_eq!(flush_log(program, true), "name? ||hello\n|a\n||");
    }

    #[test]
    fn deep_recursion_raises_a_range_error() {
        let (_, result) = run_captured(compile("func f(int n) => f(n + 1);\nfunc m{ f(0); }").unwrap(), "");
        let error = result.unwrap_err();
        assert_eq!(error.kind, ErrorKind::RangeError);
        assert!(error.message.contains("call depth"), "{error}");
    }
}
