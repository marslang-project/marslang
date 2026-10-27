//! std.process: running programs. The programs run are the marslang binary
//! itself on small scripts, so every test works the same on Windows and Linux.
//! (A few use the platform's own `cmd` or `cat`, chosen with `cfg!(windows)`.)

use std::path::PathBuf;
use std::time::{Duration, Instant};

fn executes(source: &str, expected: &str) {
    let compiled = marslang::compile(source).expect("Marslang compilation failed");
    let (stdout, result) = marslang::run_captured(compiled, "");
    if let Err(error) = result { panic!("runtime error: {}\noutput so far:\n{stdout}", error.report()); }
    assert_eq!(stdout, expected);
}

fn runtime_error(source: &str, expected: &str) {
    let compiled = marslang::compile(source).expect("compile failed");
    let (_, result) = marslang::run_captured(compiled, "");
    let error = result.expect_err(&format!("expected runtime error: {source}")).to_string();
    assert!(error.contains(expected), "expected {expected}, got {error}");
}

/// The interpreter binary, with / separators so it can go in a Marslang string.
fn exe() -> String { env!("CARGO_BIN_EXE_marslang").replace('\\', "/") }

fn temp_dir() -> PathBuf {
    let dir = std::env::temp_dir().join(format!("marslang-process-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn shown(path: &std::path::Path) -> String { path.to_string_lossy().replace('\\', "/") }

/// Write a script named for its test; its path.
fn script(name: &str, source: &str) -> String {
    let path = temp_dir().join(format!("{name}.mars"));
    std::fs::write(&path, source).unwrap();
    shown(&path)
}

/// A test program: @EXE@ is the interpreter and @SCRIPT@ the script, written
/// from the second argument.
fn with_script(program: &str, name: &str, script_source: &str) -> String {
    program.replace("@EXE@", &exe()).replace("@SCRIPT@", &script(name, script_source))
}

// ----- results -----

#[test]
fn run_gives_the_code_and_the_output() {
    executes(&with_script(r#"
        takepkg std.process;
        func m{
            r = process.run(arr("@EXE@", "@SCRIPT@"));
            out(r.code, r.ok, r.output.strip(), r.errors == "");
        }"#, "hello", r#"func m{ out("hello"); }"#),
        "0 true hello true\n");
}

#[test]
fn a_nonzero_code_is_a_result_not_an_error() {
    executes(&with_script(r#"
        takepkg std.process;
        func m{
            r = process.run(arr("@EXE@", "@SCRIPT@"));
            out(r.code, r.ok, r.output.strip());
        }"#, "exit3", r#"takepkg std.cli; func m{ out("bye"); cli.exit(3); }"#),
        "3 false bye\n");
}

#[test]
fn what_the_program_writes_to_stderr_is_in_errors() {
    executes(&with_script(r#"
        takepkg std.process;
        func m{
            r = process.run(arr("@EXE@", "@SCRIPT@"));
            out(r.ok, r.output.strip(), r.errors.contains("boom"));
        }"#, "stderr", r#"func m{ out("fine"); err(Error, "boom"); }"#),
        "false fine true\n");
}

#[test]
fn run_with_feeds_standard_input() {
    executes(&with_script(r#"
        takepkg std.process;
        func m{
            r = process.run_with(arr("@EXE@", "@SCRIPT@"), "abc é");
            out(r.output.strip());
            // Without input the program reads an empty stdin and does not wait.
            out(process.run(arr("@EXE@", "@SCRIPT@")).output.strip() == "");
        }"#, "stdin", r#"func m{ out(in().upper()); }"#),
        "ABC É\ntrue\n");
}

#[test]
fn a_large_input_does_not_deadlock_with_the_output() {
    // 4 MB in, echoed back out: the program writes while input is still being sent.
    executes(&with_script(r#"
        takepkg std.process;
        func m{
            text = "0123456789abcdef";
            for (i = 0, i < 18, i++){ text = text + text; }
            r = process.run_with(arr("@EXE@", "@SCRIPT@"), text);
            out(r.output.strip().len());
        }"#, "echo", r#"func m{ out(in()); }"#),
        "4194304\n");
}

#[test]
fn arguments_reach_the_program_unchanged_with_no_shell() {
    executes(&with_script(r#"
        takepkg std.process;
        func m{
            r = process.run(arr("@EXE@", "@SCRIPT@", "a; echo injected", "$HOME", "two words", "\"quoted\"", ""));
            out(r.output.strip());
        }"#, "args", r#"takepkg std.cli; func m{ for (a, cli.args()){ out("[" + a + "]"); } }"#),
        "[a; echo injected]\n[$HOME]\n[two words]\n[\"quoted\"]\n[]\n");
}

#[test]
fn output_that_is_not_utf8_is_replaced() {
    let file = temp_dir().join("latin1.txt");
    std::fs::write(&file, b"caf\xe9!").unwrap();
    let program = if cfg!(windows) { format!(r#"arr("cmd", "/c", "type", "{}")"#, shown(&file).replace('/', "\\\\")) }
        else { format!(r#"arr("cat", "{}")"#, shown(&file)) };
    executes(&format!(r#"
        takepkg std.process;
        func m{{
            r = process.run({program});
            out(r.output, r.output.len());
        }}"#), "caf\u{FFFD}! 5\n");
}

#[test]
fn a_platform_program_runs_too() {
    let program = if cfg!(windows) { r#"arr("cmd", "/c", "echo", "hi")"# } else { r#"arr("echo", "hi")"# };
    executes(&format!(r#"
        takepkg std.process;
        func m{{
            r = process.run({program});
            out(r.ok, r.output.strip());
        }}"#), "true hi\n");
}

// ----- check -----

#[test]
fn check_returns_the_result_or_raises_with_the_outcome() {
    executes(&with_script(r#"
        takepkg std.process;
        func m{
            run {
                process.check(arr("@EXE@", "@SCRIPT@"));
            } handle(process.ProcessError e){
                out(e.message.contains("ended with code 7"), e.message.contains("some output"), e.message.contains("boom"));
                out(e.code, e.output.strip(), e.errors.contains("boom"));
            }
        }"#, "check", r#"takepkg std.cli; func m{ out("some output"); cli.exit(7); }"#),
        "true true false
7 some output false
");
}

#[test]
fn check_includes_stderr_in_the_message() {
    let ok_script = script("checkok", "func m{ out(1); }");
    executes(&with_script(&r#"
        takepkg std.process;
        func m{
            run {
                process.check(arr("@EXE@", "@SCRIPT@"));
            } handle(process.ProcessError e){
                out(e.message.contains("errors:"), e.message.contains("boom"), e.code != 0);
            }
            r = process.check(arr("@EXE@", "@OK@"));
            out(r.ok);
        }"#.replace("@OK@", &ok_script), "checkerr", r#"func m{ err(Error, "boom"); }"#),
        "true true true\ntrue\n");
}

// ----- errors -----

#[test]
fn a_missing_program_raises_not_found_naming_it() {
    executes(r#"
        takepkg std.process;
        func m{
            run {
                process.run(arr("no-such-program-marslang"));
            } handle(process.NotFoundError e){
                out(e.message.contains("no-such-program-marslang"), e.message.contains("not found"));
            }
            run {
                process.check(arr("no-such-program-marslang", "x"));
            } handle(process.ProcessError e){
                out("also a ProcessError");
            }
        }"#, "true true\nalso a ProcessError\n");
}

#[test]
fn bad_arguments_are_rejected() {
    runtime_error("takepkg std.process; func m{ process.run(arr()); }", "RangeError");
    runtime_error(r#"takepkg std.process; func m{ process.run(arr("")); }"#, "RangeError");
    runtime_error(r#"takepkg std.process; func m{ process.run(arr("echo", 5)); }"#, "item 1 is");
    runtime_error(r#"takepkg std.process; func m{ process.run("echo hi"); }"#, "TypeError");
    runtime_error(r#"takepkg std.process; func m{ process.set_timeout(0); }"#, "RangeError");
    runtime_error(r#"takepkg std.process; func m{ process.set_timeout(-1); }"#, "RangeError");
    runtime_error(r#"takepkg std.process; func m{ process.set_timeout("soon"); }"#, "TypeError");
}

// ----- timeout -----

#[test]
fn a_program_that_runs_too_long_is_killed() {
    let started = Instant::now();
    executes(&with_script(r#"
        takepkg std.process;
        func m{
            out(process.timeout());
            process.set_timeout(0.5);
            out(process.timeout());
            run {
                process.run(arr("@EXE@", "@SCRIPT@"));
            } handle(process.TimeoutError e){
                out(e.message.contains("more than 0.5 seconds"));
            }
            run {
                process.check(arr("@EXE@", "@SCRIPT@"));
            } handle(process.ProcessError e){
                out("TimeoutError is a ProcessError");
            }
            process.set_timeout(null);
            out(process.timeout());
        }"#, "sleeper", r#"takepkg std.time; func m{ time.sleep(30); }"#),
        "null\n0.5\ntrue\nTimeoutError is a ProcessError\nnull\n");
    assert!(started.elapsed() < Duration::from_secs(15), "took {:?}", started.elapsed());
}

#[test]
fn a_program_within_the_limit_finishes_normally() {
    executes(&with_script(r#"
        takepkg std.process;
        func m{
            process.set_timeout(60);
            out(process.run(arr("@EXE@", "@SCRIPT@")).output.strip());
        }"#, "quick", r#"func m{ out("done"); }"#), "done\n");
}

// ----- folders -----

#[test]
fn run_in_uses_another_working_folder() {
    let folder = temp_dir().join("elsewhere");
    std::fs::create_dir_all(&folder).unwrap();
    let folder = std::fs::canonicalize(&folder).unwrap();
    // The folder as the program sees it, compared by its last part.
    executes(&with_script(&format!(r#"
        takepkg std.process;
        func m{{
            r = process.run_in(arr("@EXE@", "@SCRIPT@"), "{}");
            parts = r.output.strip().replace("\\", "/").split("/");
            out(parts[parts.len() - 1]);
        }}"#, shown(&folder)), "cwd", r#"takepkg std.os; func m{ out(os.cwd()); }"#),
        "elsewhere\n");
}

#[test]
fn a_missing_folder_raises_file_not_found() {
    executes(r#"
        takepkg std.process;
        takepkg std.file;
        func m{
            run {
                process.run_in(arr("echo", "x"), "/no/such/folder/for/marslang");
            } handle(file.NotFoundError e){
                out("no such folder");
            }
        }"#, "no such folder\n");
}

// ----- which -----

#[test]
fn which_finds_programs_on_the_path() {
    let platform = if cfg!(windows) { "cmd" } else { "sh" };
    executes(&format!(r#"
        takepkg std.process;
        func m{{
            found = process.which("{platform}");
            out(found != null, found.lower().contains("{platform}"));
            out(process.which("no-such-program-marslang"));
            out(process.which(""));
            // A path is itself, when it is a program.
            out(process.which("{exe}") != null);
            out(process.which("{dir}"));
        }}"#, exe = exe(), dir = shown(&temp_dir())),
        "true true\nnull\nnull\ntrue\nnull\n");
}

#[test]
fn which_and_run_agree_about_extensions_on_windows() {
    if !cfg!(windows) { return; }
    // "cmd" has no extension; PATHEXT supplies ".exe". The result runs.
    executes(r#"
        takepkg std.process;
        func m{
            full = process.which("cmd");
            out(full.lower().ends_with("cmd.exe"));
            out(process.run(arr(full, "/c", "echo", "ok")).output.strip());
        }"#, "true\nok\n");
}

// ----- the output limit -----

#[test]
fn output_over_100_mb_is_an_error() {
    let file = temp_dir().join("huge.bin");
    std::fs::File::create(&file).unwrap().set_len(101 * 1024 * 1024).unwrap();
    let program = if cfg!(windows) { format!(r#"arr("cmd", "/c", "type", "{}")"#, shown(&file).replace('/', "\\\\")) }
        else { format!(r#"arr("cat", "{}")"#, shown(&file)) };
    let started = Instant::now();
    executes(&format!(r#"
        takepkg std.process;
        func m{{
            run {{
                process.run({program});
            }} handle(process.ProcessError e){{
                out(e.message.contains("100 MB"));
            }}
        }}"#), "true\n");
    assert!(started.elapsed() < Duration::from_secs(60));
    let _ = std::fs::remove_file(&file);
}

// ----- tasks -----

#[test]
fn programs_in_tasks_run_together() {
    let program = with_script(r#"
        takepkg std.process;
        takepkg std.async;
        async func go(int n) => process.run(arr("@EXE@", "@SCRIPT@", string(n))).output.strip();
        func m{
            out(await async.all(arr(go(1), go(2), go(3))));
        }"#, "concurrent", r#"takepkg std.time; takepkg std.cli; func m{ time.sleep(0.5); out(cli.args()[0]); }"#);
    let started = Instant::now();
    executes(&program, "[\"1\", \"2\", \"3\"]\n");
    let elapsed = started.elapsed();
    // Three programs of half a second each: about half a second, not one and a half.
    assert!(elapsed >= Duration::from_millis(500), "took {elapsed:?}");
    assert!(elapsed < Duration::from_millis(1400), "took {elapsed:?}");
}

#[test]
fn other_tasks_run_while_a_program_runs() {
    executes(&with_script(r#"
        takepkg std.process;
        takepkg std.async;
        async func slow() => process.run(arr("@EXE@", "@SCRIPT@")).output.strip();
        async func ticks(){
            n = 0;
            for (i = 0, i < 5, i++){ await async.sleep(0.02); n += 1; }
            ret n;
        }
        func m{
            t = slow();
            out(await ticks());
            out(t.done());
            out(await t);
        }"#, "overlap", r#"takepkg std.time; func m{ time.sleep(1); out("finished"); }"#),
        "5\nfalse\nfinished\n");
}

#[test]
fn cancelling_a_task_kills_its_program() {
    let marker = temp_dir().join("cancel-marker.txt");
    let _ = std::fs::remove_file(&marker);
    let source = format!(r#"
        takepkg std.time;
        takepkg std.file;
        func m{{ time.sleep(1.5); file.write("{}", "survived"); }}"#, shown(&marker));
    let started = Instant::now();
    executes(&with_script(r#"
        takepkg std.process;
        takepkg std.async;
        async func go() => process.run(arr("@EXE@", "@SCRIPT@"));
        func m{
            t = go();
            await async.sleep(0.3);
            out(t.cancel());
            run { await t; } handle(CancelledError e){ out("cancelled"); }
            // Longer than the program would have needed to finish.
            await async.sleep(2.5);
        }"#, "cancelled", &source), "true\ncancelled\n");
    assert!(started.elapsed() < Duration::from_secs(10));
    assert!(!marker.exists(), "the program was not killed");
}

#[test]
fn a_timeout_in_a_task_raises_in_that_task() {
    executes(&with_script(r#"
        takepkg std.process;
        takepkg std.async;
        async func go() => process.run(arr("@EXE@", "@SCRIPT@"));
        func m{
            process.set_timeout(0.4);
            run { await go(); } handle(process.TimeoutError e){ out("timed out"); }
        }"#, "tasktimeout", r#"takepkg std.time; func m{ time.sleep(30); }"#), "timed out\n");
}
