use std::env;
use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::process::Command;

fn main() {
    if let Err(e) = run() {
        eprintln!("error: {e}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        eprintln!("Usage: marslang <file.mars> | marslang compile <file.mars> [-o out.js] | marslang lex <file.mars> | marslang repl");
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
        "repl" => repl_cmd(),
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

fn repl_cmd() -> Result<(), String> {
    println!("marslang repl (stateful). Commands: :exit, :reset, :show");
    println!("Each submitted line is appended to the current program and re-run.");

    let mut source = String::new();
    loop {
        print!("mars> ");
        io::stdout()
            .flush()
            .map_err(|e| format!("failed to flush stdout: {e}"))?;

        let mut line = String::new();
        let read = io::stdin()
            .read_line(&mut line)
            .map_err(|e| format!("failed to read line: {e}"))?;
        if read == 0 {
            println!();
            break;
        }

        let trimmed = line.trim();
        match trimmed {
            ":exit" | ":quit" => break,
            ":reset" => {
                source.clear();
                println!("state cleared");
                continue;
            }
            ":show" => {
                println!("----- source -----\n{}------------------", source);
                continue;
            }
            "" => continue,
            _ => {}
        }

        source.push_str(trimmed);
        source.push('\n');

        match marslang::compile_source_to_js(&source) {
            Ok(js) => {
                let run = Command::new("node").arg("-e").arg(&js).status();
                match run {
                    Ok(status) if status.success() => {}
                    Ok(status) => eprintln!("node exited with status: {status}"),
                    Err(_) => {
                        eprintln!("node not found; compiled JS:\n{js}");
                    }
                }
            }
            Err(e) => eprintln!("parse/compile error: {e}"),
        }
    }

    Ok(())
}
