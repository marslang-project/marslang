// Tests for std.log and its native half, rs.log.
//
// Logs go to standard error, which run_captured cannot capture, so these tests
// send lines to a temporary file with log.to_file and read them back. Each
// program strips the 20-character timestamp and its space before printing, so
// the expected output is exact; the timestamp format is checked with std.regex.

fn executes(source: &str, expected: &str) {
    let compiled = marslang::compile(source).expect("Marslang compilation failed");
    let (stdout, result) = marslang::run_captured(compiled, "");
    if let Err(error) = result { panic!("runtime error: {error}\noutput so far:\n{stdout}"); }
    assert_eq!(stdout, expected);
}

fn runtime_error(source: &str, expected: &str) {
    let compiled = marslang::compile(source).expect("compile failed");
    let (_, result) = marslang::run_captured(compiled, "");
    let error = result.expect_err(&format!("expected runtime error: {source}")).to_string();
    assert!(error.contains(expected), "expected {expected}, got {error}");
}

/// The program with PATH replaced by a quoted string holding a temporary log
/// file's path, named for the test. Forward slashes, which Marslang strings
/// take on Windows too, keep backslashes out of the source.
fn log_source(name: &str, program: &str) -> String {
    let path = std::env::temp_dir().join(format!("marslang-std-log-{name}-{}.log", std::process::id()));
    let text = path.display().to_string().replace('\\', "/");
    program.replace("PATH", &format!("\"{text}\""))
}

#[test]
fn each_level_writes_at_or_above_the_current_level() {
    let body = r#"
        takepkg std.log;
        takepkg std.file;
        func m{
            file.write(PATH, "");
            log.to_file(PATH);
            log.set_level("LEVEL");
            log.debug("d");
            log.info("i");
            log.warn("w");
            log.error("e");
            for (line, file.read_lines(PATH)){ out(line.slice(20, line.len())); }
        }
    "#;
    let cases = [
        ("debug", "DEBUG d\nINFO  i\nWARN  w\nERROR e\n"),
        ("info", "INFO  i\nWARN  w\nERROR e\n"),
        ("warn", "WARN  w\nERROR e\n"),
        ("error", "ERROR e\n"),
        ("off", ""),
    ];
    for (level, expected) in cases {
        let source = log_source(level, &body.replace("LEVEL", level));
        executes(&source, expected);
    }
}

#[test]
fn lines_have_a_timestamp_then_the_level_padded_to_five_characters() {
    let source = log_source("timestamp", r#"
        takepkg std.log;
        takepkg std.file;
        takepkg std.regex;
        func m{
            file.write(PATH, "");
            log.to_file(PATH);
            log.info("hello");
            line = file.read_lines(PATH)[0];
            out(regex.matches(r"\d{4}-\d{2}-\d{2} \d{2}:\d{2}:\d{2} INFO  hello", line));
            out(line.len());
        }
    "#);
    // A 19-character timestamp, a space, "INFO " padded, a space, then "hello".
    executes(&source, "true\n31\n");
}

#[test]
fn level_reports_the_shared_level_and_follows_set_level() {
    executes(&log_source("level", r#"
        takepkg std.log;
        takepkg std.file;
        func m{
            out(log.level());
            log.set_level("warn");
            out(log.level());
            log.set_level("off");
            out(log.level());
            log.to_file(PATH);
            file.write(PATH, "");
            log.set_level("debug");
            out(log.level());
        }
    "#), "info\nwarn\noff\ndebug\n");
}

#[test]
fn logger_lines_carry_its_name_after_the_level() {
    executes(&log_source("logger", r#"
        takepkg std.log;
        takepkg std.file;
        func m{
            file.write(PATH, "");
            log.to_file(PATH);
            db = log.Logger("db");
            db.warn("slow query");
            for (line, file.read_lines(PATH)){ out(line.slice(20, line.len())); }
        }
    "#), "WARN  [db] slow query\n");
}

#[test]
fn logger_follows_the_shared_level_until_it_sets_its_own() {
    executes(&log_source("own-level", r#"
        takepkg std.log;
        takepkg std.file;
        func m{
            file.write(PATH, "");
            log.to_file(PATH);
            log.set_level("error");
            db = log.Logger("db");
            out(db.level());
            db.warn("skipped while following the shared error level");
            db.set_level("debug");
            out(db.level());
            db.debug("written: own level is debug");
            log.warn("still skipped: the shared level is error");
            out(log.level());
            for (line, file.read_lines(PATH)){ out(line.slice(20, line.len())); }
        }
    "#), "error\ndebug\nerror\nDEBUG [db] written: own level is debug\n");
}

#[test]
fn logger_set_level_off_silences_only_that_logger() {
    executes(&log_source("logger-off", r#"
        takepkg std.log;
        takepkg std.file;
        func m{
            file.write(PATH, "");
            log.to_file(PATH);
            quiet = log.Logger("quiet");
            quiet.set_level("off");
            quiet.error("nothing");
            log.error("shared still writes");
            for (line, file.read_lines(PATH)){ out(line.slice(20, line.len())); }
        }
    "#), "ERROR shared still writes\n");
}

#[test]
fn to_screen_stops_the_file_and_to_file_can_be_chosen_again() {
    executes(&log_source("screen", r#"
        takepkg std.log;
        takepkg std.file;
        func m{
            file.write(PATH, "");
            log.to_file(PATH);
            log.info("to the file");
            log.to_screen();
            log.info("to standard error");
            log.to_file(PATH);
            log.warn("to the file again");
            for (line, file.read_lines(PATH)){ out(line.slice(20, line.len())); }
        }
    "#), "INFO  to the file\nWARN  to the file again\n");
}

#[test]
fn a_non_string_message_is_converted_with_string() {
    executes(&log_source("array", r#"
        takepkg std.log;
        takepkg std.file;
        func m{
            file.write(PATH, "");
            log.to_file(PATH);
            log.info(arr(1, 2));
            log.info(42);
            for (line, file.read_lines(PATH)){ out(line.slice(20, line.len())); }
        }
    "#), "INFO  [1, 2]\nINFO  42\n");
}

#[test]
fn an_unknown_level_name_raises_range_error_naming_the_choices() {
    runtime_error(r#"
        takepkg std.log;
        func m{ log.set_level("loud"); }
    "#, r#"log.set_level needs "debug", "info", "warn", "error", or "off", got "loud""#);
}

#[test]
fn an_unknown_logger_level_name_raises_range_error() {
    runtime_error(r#"
        takepkg std.log;
        func m{
            db = log.Logger("db");
            db.set_level("trace");
        }
    "#, r#"Logger.set_level needs "debug", "info", "warn", "error", or "off", got "trace""#);
}

#[test]
fn a_non_string_level_name_is_a_range_error_too() {
    runtime_error(r#"
        takepkg std.log;
        func m{ log.set_level(3); }
    "#, r#"log.set_level needs "debug", "info", "warn", "error", or "off", got 3"#);
}

