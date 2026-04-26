use std::env;
use std::fs;
use std::path::{Path, PathBuf};

fn main() {
    if let Err(e) = run() {
        eprintln!("error: {e}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        eprintln!("Usage: compiler <file.mrs> | compiler compile <file.mrs> [-o out.js] | compiler lex <file.mrs>");
        return Ok(());
    }

    match args[1].as_str() {
        "compile" => {
            if args.len() < 3 {
                return Err("missing input file".to_string());
            }
            let input = PathBuf::from(&args[2]);
            let output = find_output_arg(&args[3..]);
            compile_cmd(&input, output)
        }
        "lex" => {
            if args.len() < 3 {
                return Err("missing input file".to_string());
            }
            lex_cmd(Path::new(&args[2]))
        }
        _ => compile_cmd(Path::new(&args[1]), None),
    }
}

fn find_output_arg(rest: &[String]) -> Option<PathBuf> {
    let mut i = 0;
    while i + 1 < rest.len() {
        if rest[i] == "-o" || rest[i] == "--output" {
            return Some(PathBuf::from(&rest[i + 1]));
        }
        i += 1;
    }
    None
}

fn compile_cmd(input: &Path, output: Option<PathBuf>) -> Result<(), String> {
    let source = fs::read_to_string(input)
        .map_err(|e| format!("failed to read source file {}: {e}", input.display()))?;
    let js = marslang::compile_source_to_js(&source)?;

    let output = output.unwrap_or_else(|| input.with_extension("js"));
    fs::write(&output, js)
        .map_err(|e| format!("failed to write output file {}: {e}", output.display()))?;
    println!("compiled {} -> {}", input.display(), output.display());
    Ok(())
}

fn lex_cmd(input: &Path) -> Result<(), String> {
    let source = fs::read_to_string(input)
        .map_err(|e| format!("failed to read source file {}: {e}", input.display()))?;
    for tok in marslang::lexer::lex(&source) {
        println!("{:?} @{}", tok.kind, tok.pos);
    }
    Ok(())
}
