//! `takepkg rs.process;` - the native side of `std.process`: starting another
//! program (no shell: the arguments go to it as they are), feeding its input,
//! collecting its output, and killing it when it runs too long.
//!
//! Rust, because Marslang cannot start programs. Everything else, the `Result`
//! family, the error families, and the checking wrappers, is Marslang, in
//! `std/process.mars`.
//!
//! A program runs on a worker thread (`runtime::block_on_worker`), so inside a
//! task only that task waits and the others run; outside a task it simply
//! blocks. Only `Send` data crosses to the worker: strings and numbers, never a
//! `Value`. A task cancelled while its program runs sets a flag when it is
//! dropped, and the worker kills the program within a few milliseconds.
//!
//! `run` returns `[true, [code, output, errors]]` or `[false, kind, message]`,
//! and `std/process.mars` raises the family `kind` names (a native package can
//! raise only built-in errors).

use std::cell::Cell;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc, Arc};
use std::time::{Duration, Instant};

use crate::runtime;
use crate::value::*;

/// Each of a program's two output streams may hold this much.
const MAX_OUTPUT: usize = 100 * 1024 * 1024;

thread_local! {
    /// How long a program may run; none means as long as it likes. Read on the
    /// interpreter's thread and sent along with each program.
    static TIMEOUT: Cell<Option<Duration>> = const { Cell::new(None) };
}

// ----- looking programs up -----

/// Whether a name holds a folder part: `tools/run`, or `tools\run` on Windows.
fn has_separator(name: &str) -> bool {
    name.contains('/') || (cfg!(windows) && name.contains('\\'))
}

/// The extensions that make a file a program on Windows, from PATHEXT, in
/// order, lower case and with the dot; none elsewhere.
fn program_extensions() -> Vec<String> {
    if !cfg!(windows) { return Vec::new(); }
    let list = std::env::var("PATHEXT").ok().filter(|v| !v.trim().is_empty())
        .unwrap_or_else(|| ".COM;.EXE;.BAT;.CMD".to_string());
    list.split(';').map(|e| e.trim().to_ascii_lowercase()).filter(|e| e.starts_with('.') && e.len() > 1).collect()
}

/// Whether `path` is a file the system would run.
fn is_program(path: &Path) -> bool {
    let Ok(meta) = std::fs::metadata(path) else { return false };
    if !meta.is_file() { return false; }
    #[cfg(unix)]
    { std::os::unix::fs::PermissionsExt::mode(&meta.permissions()) & 0o111 != 0 }
    #[cfg(not(unix))]
    { true }
}

/// The ways `base` can name a program: itself, and on Windows with each of the
/// PATHEXT extensions added (itself only when it already ends in one).
fn variants(base: PathBuf, extensions: &[String]) -> Vec<PathBuf> {
    if extensions.is_empty() { return vec![base]; }
    let name = base.file_name().map(|n| n.to_string_lossy().to_ascii_lowercase()).unwrap_or_default();
    let mut found = Vec::new();
    if extensions.iter().any(|e| name.ends_with(e.as_str())) { found.push(base.clone()); }
    for extension in extensions {
        let mut with = base.clone().into_os_string();
        with.push(extension);
        found.push(PathBuf::from(with));
    }
    found
}

fn absolute(path: PathBuf) -> PathBuf {
    std::path::absolute(&path).unwrap_or(path)
}

/// The program `name` names, as the system would find it: a name with a folder
/// part is that path (relative to the current folder); any other name is
/// searched for in the PATH folders in order. On Windows a name without an
/// extension is tried with each PATHEXT extension, so `npm` finds `npm.cmd`.
/// The path is absolute. None when there is no such program.
pub fn find(name: &str) -> Option<PathBuf> {
    if name.is_empty() || name.contains('\0') { return None; }
    let extensions = program_extensions();
    if has_separator(name) {
        return variants(PathBuf::from(name), &extensions).into_iter().find(|p| is_program(p)).map(absolute);
    }
    let path = std::env::var_os("PATH")?;
    for folder in std::env::split_paths(&path) {
        if folder.as_os_str().is_empty() { continue; }
        if let Some(found) = variants(folder.join(name), &extensions).into_iter().find(|p| is_program(p)) {
            return Some(absolute(found));
        }
    }
    None
}

// ----- running -----

/// What to run, checked on the interpreter's thread and made of `Send` parts.
struct Spec { args: Vec<String>, folder: Option<String>, input: Option<String>, timeout: Option<Duration> }

/// A finished program.
struct Done { code: Option<i32>, output: Vec<u8>, errors: Vec<u8> }

/// A kind `std/process.mars` turns into an error family, and the message.
type Failure = (&'static str, String);

fn seconds(duration: Duration) -> String {
    let s = duration.as_secs_f64();
    if s == 1.0 { "1 second".into() } else { format!("{s} seconds") }
}

/// Collect a stream into memory, up to the limit; the bytes arrive on `done`
/// when the stream ends. Past the limit it sets `exceeded` and stops reading.
fn collect(mut source: impl Read + Send + 'static, which: u8, exceeded: Arc<AtomicBool>, done: mpsc::Sender<(u8, Vec<u8>)>) {
    std::thread::spawn(move || {
        let mut bytes = Vec::new();
        let mut buffer = vec![0u8; 64 * 1024];
        loop {
            match source.read(&mut buffer) {
                Ok(0) => break,
                Ok(n) => {
                    if bytes.len() + n > MAX_OUTPUT { exceeded.store(true, Ordering::Relaxed); break; }
                    bytes.extend_from_slice(&buffer[..n]);
                }
                Err(e) if e.kind() == std::io::ErrorKind::Interrupted => {}
                Err(_) => break,
            }
        }
        let _ = done.send((which, bytes));
    });
}

fn kill(child: &mut Child) {
    let _ = child.kill();
    let _ = child.wait();
}

/// Start the program and wait for it. Runs on a worker thread; `stop` is set
/// when the task that wanted the answer has gone, and kills the program.
fn execute(spec: Spec, stop: &AtomicBool) -> Result<Done, Failure> {
    let program = &spec.args[0];
    if let Some(folder) = &spec.folder {
        let path = Path::new(folder);
        if !path.exists() { return Err(("FolderMissing", format!("{folder}: no such folder"))); }
        if !path.is_dir() { return Err(("FolderNotDirectory", format!("{folder}: is not a folder"))); }
    }
    // A relative path is taken from this program's folder on every system, not
    // from the new one on Linux and the old one on Windows. On Windows a
    // program is looked up with PATHEXT first (Command adds only ".exe").
    let resolved = if has_separator(program) || cfg!(windows) {
        find(program).unwrap_or_else(|| PathBuf::from(program))
    } else {
        PathBuf::from(program)
    };
    let mut command = Command::new(&resolved);
    command.args(&spec.args[1..]);
    if let Some(folder) = &spec.folder { command.current_dir(folder); }
    command.stdin(if spec.input.is_some() { Stdio::piped() } else { Stdio::null() })
        .stdout(Stdio::piped()).stderr(Stdio::piped());
    let mut child = command.spawn().map_err(|error| {
        use std::io::ErrorKind::*;
        // A PATH folder this user cannot search makes the system answer
        // "permission denied" for a name that is simply nowhere on PATH.
        let missing = error.kind() == NotFound
            || (error.kind() == PermissionDenied && !has_separator(program) && find(program).is_none());
        match error.kind() {
            _ if missing => ("NotFound", format!(
                "the program \"{program}\" was not found; give the name of an installed program (looked up in PATH) or a path to one")),
            PermissionDenied => ("Process", format!("the program \"{program}\" cannot be run: permission denied (is it a program, and executable?)")),
            _ => ("Process", format!("the program \"{program}\" could not be started: {error}")),
        }
    })?;

    if let (Some(mut stdin), Some(input)) = (child.stdin.take(), spec.input) {
        // A program that never reads its input closes the pipe: not an error.
        std::thread::spawn(move || { let _ = stdin.write_all(input.as_bytes()); });
    }
    let exceeded = Arc::new(AtomicBool::new(false));
    let (done, finished) = mpsc::channel();
    if let Some(out) = child.stdout.take() { collect(out, 0, exceeded.clone(), done.clone()); }
    if let Some(err) = child.stderr.take() { collect(err, 1, exceeded.clone(), done.clone()); }
    drop(done);

    let deadline = spec.timeout.map(|t| Instant::now() + t);
    let timed_out = |child: &mut Child| {
        kill(child);
        let limit = spec.timeout.map(seconds).unwrap_or_default();
        ("Timeout", format!("the program \"{program}\" ran for more than {limit} and was killed (process.set_timeout changes the limit)"))
    };
    let too_much = |child: &mut Child| {
        kill(child);
        ("Process", format!("the program \"{program}\" wrote more than {} MB to its output or errors, and was killed", MAX_OUTPUT >> 20))
    };

    // Poll for the end, quickly at first and then every 10 ms: waiting for a
    // child has no portable form that also watches a flag and a clock.
    let mut pause = Duration::from_millis(1);
    let status = loop {
        if stop.load(Ordering::Relaxed) {
            kill(&mut child);
            return Err(("Process", format!("the program \"{program}\" was killed because its task was cancelled")));
        }
        if exceeded.load(Ordering::Relaxed) { return Err(too_much(&mut child)); }
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) => {}
            Err(error) => {
                kill(&mut child);
                return Err(("Process", format!("the program \"{program}\" could not be waited for: {error}")));
            }
        }
        if deadline.is_some_and(|d| Instant::now() >= d) { return Err(timed_out(&mut child)); }
        std::thread::sleep(pause);
        pause = (pause * 2).min(Duration::from_millis(10));
    };

    // The program has ended; its streams end with it, unless a program it
    // started keeps them open, which this waits for, within the same limits.
    let (mut output, mut errors, mut ended) = (Vec::new(), Vec::new(), 0);
    while ended < 2 {
        match finished.recv_timeout(Duration::from_millis(10)) {
            Ok((0, bytes)) => { output = bytes; ended += 1; }
            Ok((_, bytes)) => { errors = bytes; ended += 1; }
            Err(mpsc::RecvTimeoutError::Disconnected) => break,
            Err(mpsc::RecvTimeoutError::Timeout) => {
                if stop.load(Ordering::Relaxed) { return Err(("Process", format!("the program \"{program}\" was cancelled"))); }
                if deadline.is_some_and(|d| Instant::now() >= d) { return Err(timed_out(&mut child)); }
            }
        }
    }
    if exceeded.load(Ordering::Relaxed) { return Err(too_much(&mut child)); }
    Ok(Done { code: status.code(), output, errors })
}

/// Sets a flag when dropped: a task waiting for a program that is cancelled,
/// or unwound at the end of the run, tells the worker to kill the program.
struct StopOnDrop(Arc<AtomicBool>);

impl Drop for StopOnDrop {
    fn drop(&mut self) { self.0.store(true, Ordering::Relaxed); }
}

fn strings(value: &Value) -> RResult<Vec<String>> {
    let Value::Array(array) = value else {
        return type_err(format!("process.run expects the arguments as an array of strings, got {}", value.type_name()));
    };
    let items = array.items.borrow();
    if items.is_empty() { return range_err("process.run needs at least the program to run, but the array of arguments is empty"); }
    items.iter().enumerate().map(|(i, item)| match item {
        Value::Str(text) => Ok(text.to_string()),
        other => type_err(format!("process.run expects every argument to be a string, but item {i} is {}", other.type_name())),
    }).collect()
}

fn optional_text(value: &Value, what: &str) -> RResult<Option<String>> {
    match value {
        Value::Null => Ok(None),
        Value::Str(text) => Ok(Some(text.to_string())),
        other => type_err(format!("process.run expects {what} as a string, got {}", other.type_name())),
    }
}

fn run(args: &[Value]) -> RResult<Value> {
    let args_list = strings(&args[0])?;
    if args_list[0].is_empty() { return range_err("process.run needs a program name, but the first argument is an empty string"); }
    let spec = Spec {
        args: args_list,
        folder: optional_text(&args[1], "the folder")?,
        input: optional_text(&args[2], "the input")?,
        timeout: TIMEOUT.with(Cell::get),
    };
    let stop = Arc::new(AtomicBool::new(false));
    let guard = StopOnDrop(stop.clone());
    let result = runtime::block_on_worker(move || execute(spec, &stop))?;
    drop(guard);
    Ok(match result {
        Ok(done) => Value::array(vec![Value::Bool(true), Value::array(vec![
            done.code.map_or(Value::Null, |c| Value::Int(c as i64)),
            Value::str(&String::from_utf8_lossy(&done.output)),
            Value::str(&String::from_utf8_lossy(&done.errors)),
        ])]),
        Err((kind, message)) => Value::array(vec![Value::Bool(false), Value::str(kind), Value::str(&message)]),
    })
}

// ----- the package -----

pub fn package() -> Value {
    NativePackage::new("rs.process")
        // Run a program: [args, folder or null, input or null].
        .function("run", 3, run)
        // The full path of a program on PATH, or null.
        .function("which", 1, |a| match &a[0] {
            Value::Str(name) => Ok(find(name).map_or(Value::Null, |p| Value::str(&p.to_string_lossy()))),
            other => type_err(format!("process.which expects a program name as a string, got {}", other.type_name())),
        })
        .function("set_timeout", 1, |a| {
            let seconds = match &a[0] {
                Value::Null => { TIMEOUT.with(|t| t.set(None)); return Ok(Value::Null); }
                Value::Int(n) | Value::Long(n) => *n as f64,
                Value::Float(f) => *f,
                other => return type_err(format!("process.set_timeout expects a number of seconds or null, got {}", other.type_name())),
            };
            match Duration::try_from_secs_f64(seconds) {
                Ok(duration) if seconds > 0.0 && seconds <= 1e9 => { TIMEOUT.with(|t| t.set(Some(duration))); Ok(Value::Null) }
                _ => range_err(format!("process.set_timeout needs a positive number of seconds, or null for no limit, got {seconds}")),
            }
        })
        .function("timeout", 0, |_| Ok(TIMEOUT.with(Cell::get).map_or(Value::Null, |d| Value::Float(d.as_secs_f64()))))
        .build()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_with_a_folder_part_are_paths() {
        assert!(has_separator("tools/run"));
        assert!(!has_separator("run"));
        assert_eq!(has_separator("tools\\run"), cfg!(windows));
    }

    #[test]
    fn nothing_is_found_for_an_empty_or_unknown_name() {
        assert!(find("").is_none());
        assert!(find("no-such-program-marslang-test").is_none());
    }

    #[test]
    fn windows_tries_pathext_extensions() {
        let extensions = vec![".exe".to_string(), ".cmd".to_string()];
        let tried: Vec<String> = variants(PathBuf::from("npm"), &extensions).iter().map(|p| p.to_string_lossy().into_owned()).collect();
        assert_eq!(tried, ["npm.exe", "npm.cmd"]);
        let tried: Vec<String> = variants(PathBuf::from("Tool.CMD"), &extensions).iter().map(|p| p.to_string_lossy().into_owned()).collect();
        assert_eq!(tried, ["Tool.CMD", "Tool.CMD.exe", "Tool.CMD.cmd"]);
        assert_eq!(variants(PathBuf::from("ls"), &[]).len(), 1);
    }
}
