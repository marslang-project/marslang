use std::env;
use std::fs;
use std::io::{self, Write};
use std::path::Path;
use std::process::ExitCode;

const USAGE: &str = "Usage: marslang <file.mars> | marslang run <file.mars> | marslang check <file.mars> | marslang lex <file.mars> | marslang repl | marslang --version";

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("error: {e}");
            ExitCode::FAILURE
        }
    }
}

fn run() -> Result<(), String> {
    let args: Vec<String> = env::args().collect();
    let file = |i: usize| args.get(i).map(Path::new).ok_or_else(|| "missing input file".to_string());
    match args.get(1).map(String::as_str) {
        None | Some("-h" | "--help" | "help") => {
            eprintln!("{USAGE}");
            Ok(())
        }
        Some("-V" | "--version" | "version") => {
            println!("marslang {}", marslang::VERSION);
            Ok(())
        }
        Some("run") => run_cmd(file(2)?),
        Some("check") => {
            marslang::compile_file(file(2)?)?;
            println!("ok");
            Ok(())
        }
        Some("lex") => lex_cmd(file(2)?),
        Some("repl") => repl_cmd(),
        Some(path) => run_cmd(Path::new(path)),
    }
}

fn read(input: &Path) -> Result<String, String> {
    fs::read_to_string(input).map_err(|e| format!("failed to read source file {}: {e}", input.display()))
}

fn run_cmd(input: &Path) -> Result<(), String> {
    let compiled = marslang::compile_file(input)?;
    marslang::run(compiled).map_err(|e| e.to_string())
}

fn lex_cmd(input: &Path) -> Result<(), String> {
    for tok in marslang::lexer::lex(&read(input)?) {
        println!("{:?} @{}", tok.kind, tok.pos);
    }
    Ok(())
}

/// The REPL keeps the submitted program and replays it after each line, showing
/// only output that the new line produced. Lines that fail are discarded.
fn repl_cmd() -> Result<(), String> {
    println!("marslang {} repl. Commands: :exit, :reset, :show", marslang::VERSION);
    println!("Each line is added to the program, which is re-run; only new output is shown.");

    let mut source = String::new();
    let mut shown = 0usize;
    loop {
        print!("mars> ");
        io::stdout().flush().map_err(|e| format!("failed to flush stdout: {e}"))?;

        let mut line = String::new();
        let read = io::stdin().read_line(&mut line).map_err(|e| format!("failed to read line: {e}"))?;
        if read == 0 {
            println!();
            break;
        }

        let trimmed = line.trim();
        match trimmed {
            ":exit" | ":quit" => break,
            ":reset" => {
                source.clear();
                shown = 0;
                println!("state cleared");
                continue;
            }
            ":show" => {
                println!("----- source -----\n{source}------------------");
                continue;
            }
            "" => continue,
            _ => {}
        }

        let candidate = format!("{source}{trimmed}\n");
        let compiled = match marslang::compile(&candidate) {
            Ok(compiled) => compiled,
            Err(e) => {
                eprintln!("error: {e}");
                continue;
            }
        };
        let (output, result) = marslang::run_captured(compiled, "");
        print!("{}", output.get(shown..).unwrap_or(&output));
        match result {
            Ok(()) => {
                source = candidate;
                shown = output.len();
            }
            Err(e) => eprintln!("error: {e}"),
        }
    }

    Ok(())
}
