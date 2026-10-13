use std::env;
use std::fs;
use std::io::{self, Write};
use std::path::Path;
use std::process::ExitCode;

// Static musl builds, the Linux releases, use mimalloc: musl's allocator is
// much slower for an interpreter that allocates as often as this one.
#[cfg(target_env = "musl")]
#[global_allocator]
static ALLOCATOR: mimalloc::MiMalloc = mimalloc::MiMalloc;

const USAGE: &str = "Usage: marslang <file.mars> [args...] | marslang run <file.mars> [args...] | marslang test [file or folder] | marslang check <file.mars> | marslang lex <file.mars> | marslang repl | marslang pkgs | marslang symbols <file.mars> [--stdin] | marslang --version";

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
        Some("run") => run_cmd(file(2)?, &args[3.min(args.len())..]),
        Some("test") => test_cmd(args.get(2).map(Path::new)),
        Some("check") => {
            marslang::check_file(file(2)?)?;
            println!("ok");
            Ok(())
        }
        Some("lex") => lex_cmd(file(2)?),
        // Declarations as JSON, for editors. With --stdin the source is read
        // from standard input (unsaved edits), and packages from the file's folder.
        Some("symbols") => {
            let path = file(2)?;
            let source = if args.get(3).map(String::as_str) == Some("--stdin") {
                let mut text = String::new();
                io::Read::read_to_string(&mut io::stdin(), &mut text).map_err(|e| format!("failed to read standard input: {e}"))?;
                text
            } else {
                read(path)?
            };
            let base = path.parent().filter(|p| !p.as_os_str().is_empty()).unwrap_or(Path::new("."));
            println!("{}", marslang::symbols(&source, base)?);
            Ok(())
        }
        // Where packages installed for this user are imported from, so an
        // installation can be checked without running a program.
        Some("pkgs") => {
            match marslang::user_packages() {
                Some(path) => println!("{}", path.display()),
                None => return Err("no user package directory: set MARSLANG_PKGS".into()),
            }
            Ok(())
        }
        Some("repl") => repl_cmd(),
        Some(path) => run_cmd(Path::new(path), &args[2..]),
    }
}

fn read(input: &Path) -> Result<String, String> {
    fs::read_to_string(input).map_err(|e| format!("failed to read source file {}: {e}", input.display()))
}

/// Run a program; the words after its file are its arguments (`std.cli.args()`).
fn run_cmd(input: &Path, program_args: &[String]) -> Result<(), String> {
    let compiled = marslang::compile_file(input)?;
    match marslang::run_with_args(compiled, input.display().to_string(), program_args.to_vec()) {
        // cli.exit(code): output is already flushed, so stop with that status.
        Err(error) if error.exit.is_some() => std::process::exit(error.exit.unwrap()),
        result => result.map_err(|e| e.report()),
    }
}

/// `marslang test`: run every test in one file, or in every `*_test.mars`
/// file under a folder (the current one by default). A test is a top-level
/// function named `test_...` that takes no parameters; what a passing test
/// prints is hidden. Ends with status 1 when anything failed.
fn test_cmd(target: Option<&Path>) -> Result<(), String> {
    let target = target.unwrap_or(Path::new("."));
    let files = if target.is_file() {
        vec![target.to_path_buf()]
    } else if target.is_dir() {
        let mut found = Vec::new();
        find_tests(target, &mut found);
        found
    } else {
        return Err(format!("{} is neither a test file nor a folder", target.display()));
    };
    if files.is_empty() {
        return Err(format!("no tests in {}: test files are named like math_test.mars", target.display()));
    }
    let started = std::time::Instant::now();
    let (mut passed, mut failed, mut broken) = (0usize, 0usize, 0usize);
    let indent = |text: &str, by: &str| text.lines().map(|line| format!("{by}{line}")).collect::<Vec<_>>().join("\n");
    for file in &files {
        let shown = file.strip_prefix(".").unwrap_or(file).display().to_string();
        println!("{shown}");
        let outcomes = marslang::compile_file(file)
            .map_err(|error| format!("error: {error}"))
            .and_then(|compiled| marslang::run_tests(compiled, shown.clone())
                .map_err(|error| format!("error before the tests ran: {}", error.report())));
        match outcomes {
            Err(problem) => {
                broken += 1;
                println!("{}", indent(&problem, "  "));
            }
            Ok(outcomes) if outcomes.is_empty() => println!("  no tests: name them test_..., with no parameters"),
            Ok(outcomes) => for outcome in outcomes {
                match &outcome.error {
                    None => {
                        passed += 1;
                        println!("  ok    {}", outcome.name);
                    }
                    Some(error) => {
                        failed += 1;
                        println!("  FAIL  {}", outcome.name);
                        let report = match error.exit {
                            Some(code) => format!("the test called cli.exit({code})"),
                            None => error.report(),
                        };
                        println!("{}", indent(&report, "        "));
                        if !outcome.output.is_empty() {
                            println!("        it printed:\n{}", indent(outcome.output.trim_end(), "          "));
                        }
                    }
                }
            },
        }
    }
    let mut summary = format!("{passed} passed, {failed} failed");
    if broken > 0 { summary += &format!(", {broken} file{} could not run", if broken == 1 { "" } else { "s" }); }
    println!("\n{summary} in {:.2}s", started.elapsed().as_secs_f64());
    if failed + broken > 0 {
        io::stdout().flush().map_err(|e| format!("failed to flush stdout: {e}"))?;
        std::process::exit(1);
    }
    Ok(())
}

/// Every `*_test.mars` file under `dir`, sorted, leaving out hidden folders
/// and folders of build output and installed packages.
fn find_tests(dir: &Path, found: &mut Vec<std::path::PathBuf>) {
    let Ok(entries) = fs::read_dir(dir) else { return };
    let mut paths: Vec<std::path::PathBuf> = entries.filter_map(|entry| entry.ok().map(|e| e.path())).collect();
    paths.sort();
    for path in paths {
        let name = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
        if path.is_dir() {
            if !name.starts_with('.') && !matches!(name.as_str(), "target" | "node_modules" | "marslang_pkgs") {
                find_tests(&path, found);
            }
        } else if name.ends_with("_test.mars") {
            found.push(path);
        }
    }
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
            Err(e) => eprintln!("error: {}", e.report()),
        }
    }

    Ok(())
}
