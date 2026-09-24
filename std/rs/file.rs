//! `takepkg rs.file;` - the file system, for `std.file`, `std.file.path`, and
//! `std.file.csv`.
//!
//! Operations that can fail return `[true, value]` or `[false, kind, message]`,
//! and the Marslang wrapper raises the family `kind` names (`NotFound` is
//! `file.NotFoundError`), because a native package can only raise the built-in
//! error kinds itself. Every message starts with the path as it was given.

use std::fs;
use std::io::{self, BufRead, Seek, SeekFrom, Write};
use std::path::{Component, Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use crate::value::*;

// ----- results -----

fn ok(value: Value) -> RResult<Value> { Ok(Value::array(vec![Value::Bool(true), value])) }

fn fail(kind: &str, message: impl Into<String>) -> RResult<Value> {
    Ok(Value::array(vec![Value::Bool(false), Value::str(kind), Value::str(&message.into())]))
}

/// An I/O error as a failure, worded for the path it happened to.
fn io_fail(path: &str, error: io::Error) -> RResult<Value> {
    use io::ErrorKind::*;
    match error.kind() {
        NotFound => fail("NotFound", format!("{path}: no such file or directory")),
        PermissionDenied => fail("Permission", format!("{path}: permission denied")),
        AlreadyExists => fail("Exists", format!("{path}: already exists")),
        IsADirectory => fail("IsDirectory", format!("{path}: is a directory")),
        NotADirectory => fail("NotDirectory", format!("{path}: a part of this path is a file, not a directory")),
        DirectoryNotEmpty => fail("File", format!("{path}: the directory is not empty; file.remove_all removes it with everything in it")),
        _ => fail("File", format!("{path}: {error}")),
    }
}

/// Unwrap an I/O result inside a function that returns a failure value.
macro_rules! attempt {
    ($path:expr, $result:expr) => {
        match $result { Ok(value) => value, Err(error) => return io_fail($path, error) }
    };
}

fn text<'a>(value: &'a Value, what: &str) -> RResult<&'a str> {
    match value {
        Value::Str(text) => Ok(text),
        other => type_err(format!("{what} expects a string, got {}", other.type_name())),
    }
}

fn number(n: u64) -> Value {
    match i32::try_from(n) { Ok(small) => Value::Int(small as i64), Err(_) => Value::Long(n as i64) }
}

fn display(path: &Path) -> Value { Value::str(&path.to_string_lossy()) }

/// Bytes as text, or a failure naming the first byte that is not UTF-8.
fn utf8(path: &str, bytes: Vec<u8>, offset: u64) -> Result<String, RResult<Value>> {
    String::from_utf8(bytes).map_err(|e| {
        let at = e.utf8_error().valid_up_to();
        let byte = e.as_bytes()[at];
        fail("File", format!(
            "{path}: the file is not UTF-8 text (byte 0x{byte:02X} at offset {}); std.file reads text files only",
            offset + at as u64
        ))
    })
}

fn is_dir(path: &str) -> bool { fs::metadata(path).is_ok_and(|m| m.is_dir()) }

// ----- reading -----

fn read(path: &str) -> RResult<Value> {
    if is_dir(path) { return fail("IsDirectory", format!("{path}: is a directory; file.list shows what is inside")); }
    let bytes = attempt!(path, fs::read(path));
    match utf8(path, bytes, 0) { Ok(text) => ok(Value::str(&text)), Err(failure) => failure }
}

fn read_lines(path: &str) -> RResult<Value> {
    let result = read(path)?;
    let Value::Array(parts) = &result else { unreachable!() };
    let parts = parts.items.borrow();
    if matches!(parts[0], Value::Bool(false)) { return Ok(result.clone()); }
    let Value::Str(text) = &parts[1] else { unreachable!() };
    // str::lines drops "\n" and "\r\n", and a final line ending adds no empty line.
    ok(Value::array(text.lines().map(Value::str).collect()))
}

/// Up to `count` lines from byte `offset`, and the offset after them, or -1
/// at the end of the file. `std.file.each_line` calls this until the end, so
/// only one chunk of a large file is in memory at a time.
fn lines_chunk(path: &str, offset: &Value, count: usize) -> RResult<Value> {
    let start = match offset {
        Value::Int(n) | Value::Long(n) if *n >= 0 => *n as u64,
        _ => return range_err("lines_chunk expects a nonnegative offset"),
    };
    if is_dir(path) { return fail("IsDirectory", format!("{path}: is a directory")); }
    let mut file = attempt!(path, fs::File::open(path));
    attempt!(path, file.seek(SeekFrom::Start(start)));
    let mut reader = io::BufReader::new(file);
    let mut at = start;
    let mut lines = Vec::new();
    while lines.len() < count {
        let mut bytes = Vec::new();
        let read = attempt!(path, reader.read_until(b'\n', &mut bytes));
        if read == 0 { return ok(Value::array(vec![Value::array(lines), Value::Int(-1)])); }
        if bytes.ends_with(b"\n") { bytes.pop(); }
        if bytes.ends_with(b"\r") { bytes.pop(); }
        match utf8(path, bytes, at) {
            Ok(line) => lines.push(Value::str(&line)),
            Err(failure) => return failure,
        }
        at += read as u64;
    }
    ok(Value::array(vec![Value::array(lines), number(at)]))
}

// ----- writing -----

static TEMPORARY: AtomicU64 = AtomicU64::new(0);

/// The directory a new file at `path` goes into, when it exists.
fn check_parent(path: &str) -> Option<RResult<Value>> {
    if is_dir(path) {
        return Some(fail("IsDirectory", format!("{path}: is a directory")));
    }
    let parent = Path::new(path).parent().filter(|p| !p.as_os_str().is_empty())?;
    if !parent.is_dir() {
        return Some(fail("NotFound", format!(
            "{path}: the directory {} does not exist; file.make_dir creates it", parent.display()
        )));
    }
    None
}

/// Replace the file all at once: write a temporary file beside it, flush it
/// to disk, then rename it over the old one, so a crash or a full disk leaves
/// either the old contents or the new, never half of either.
fn write(path: &str, contents: &[u8]) -> RResult<Value> {
    if let Some(failure) = check_parent(path) { return failure; }
    let target = Path::new(path);
    let name = target.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    let temporary = target.with_file_name(format!(
        ".{name}.{}.{}.tmp", std::process::id(), TEMPORARY.fetch_add(1, Ordering::Relaxed)
    ));
    let written = (|| {
        let mut file = fs::File::create(&temporary)?;
        file.write_all(contents)?;
        file.sync_all()?;
        drop(file);
        fs::rename(&temporary, target)
    })();
    if let Err(error) = written {
        let _ = fs::remove_file(&temporary);
        return io_fail(path, error);
    }
    ok(Value::Null)
}

fn append(path: &str, contents: &str) -> RResult<Value> {
    if let Some(failure) = check_parent(path) { return failure; }
    let mut file = attempt!(path, fs::OpenOptions::new().append(true).create(true).open(path));
    attempt!(path, file.write_all(contents.as_bytes()));
    ok(Value::Null)
}

fn create(path: &str, contents: &str) -> RResult<Value> {
    if let Some(failure) = check_parent(path) { return failure; }
    let mut file = match fs::OpenOptions::new().write(true).create_new(true).open(path) {
        Ok(file) => file,
        Err(e) if e.kind() == io::ErrorKind::AlreadyExists => {
            return fail("Exists", format!("{path}: already exists; file.create never replaces a file, file.write does"));
        }
        Err(e) => return io_fail(path, e),
    };
    attempt!(path, file.write_all(contents.as_bytes()));
    ok(Value::Null)
}

fn write_lines(path: &str, lines: &Value) -> RResult<Value> {
    let Value::Array(lines) = lines else { return type_err("file.write_lines expects an array of strings") };
    let mut contents = String::new();
    for line in lines.items.borrow().iter() {
        let Value::Str(line) = line else {
            return type_err(format!("file.write_lines expects an array of strings, but it holds a {}", line.type_name()));
        };
        contents.push_str(line);
        contents.push('\n');
    }
    write(path, contents.as_bytes())
}

// ----- checking -----

fn metadata(path: &str) -> Result<fs::Metadata, RResult<Value>> {
    fs::metadata(path).map_err(|e| io_fail(path, e))
}

fn size(path: &str) -> RResult<Value> {
    match metadata(path) {
        Ok(m) if m.is_dir() => fail("IsDirectory", format!("{path}: is a directory, which has no size of its own")),
        Ok(m) => ok(number(m.len())),
        Err(failure) => failure,
    }
}

fn modified(path: &str) -> RResult<Value> {
    let m = match metadata(path) { Ok(m) => m, Err(failure) => return failure };
    let when = attempt!(path, m.modified());
    let seconds = match when.duration_since(std::time::UNIX_EPOCH) {
        Ok(after) => after.as_secs_f64(),
        Err(before) => -before.duration().as_secs_f64(),
    };
    ok(Value::Float(seconds))
}

// ----- directories -----

fn list(path: &str) -> RResult<Value> {
    match metadata(path) {
        Ok(m) if !m.is_dir() => return fail("NotDirectory", format!("{path}: is a file, not a directory")),
        Ok(_) => {}
        Err(failure) => return failure,
    }
    let mut names = Vec::new();
    for entry in attempt!(path, fs::read_dir(path)) {
        let entry = attempt!(path, entry);
        names.push(entry.file_name().to_string_lossy().into_owned());
    }
    names.sort();
    ok(Value::array(names.iter().map(|n| Value::str(n)).collect()))
}

/// Every file below `dir`. A link to a directory is not followed, so a link
/// back up the tree cannot loop.
fn walk(path: &str) -> RResult<Value> {
    match metadata(path) {
        Ok(m) if !m.is_dir() => return fail("NotDirectory", format!("{path}: is a file, not a directory")),
        Ok(_) => {}
        Err(failure) => return failure,
    }
    let mut files = Vec::new();
    let mut pending = vec![PathBuf::from(path)];
    while let Some(dir) = pending.pop() {
        let shown = dir.to_string_lossy().into_owned();
        for entry in attempt!(&shown, fs::read_dir(&dir)) {
            let entry = attempt!(&shown, entry);
            let kind = attempt!(&shown, entry.file_type());
            if kind.is_dir() { pending.push(entry.path()); } else { files.push(entry.path()); }
        }
    }
    files.sort();
    ok(Value::array(files.iter().map(|p| display(p)).collect()))
}

fn make_dir(path: &str) -> RResult<Value> {
    if Path::new(path).exists() && !is_dir(path) {
        return fail("Exists", format!("{path}: a file is already there"));
    }
    attempt!(path, fs::create_dir_all(path));
    ok(Value::Null)
}

// ----- copying, moving, removing -----

fn copy(from: &str, to: &str) -> RResult<Value> {
    if is_dir(from) { return fail("IsDirectory", format!("{from}: is a directory; file.copy copies files")); }
    if is_dir(to) { return fail("IsDirectory", format!("{to}: is a directory; give the full path of the new file")); }
    if let Some(failure) = check_parent(to) { return failure; }
    attempt!(from, fs::metadata(from));
    attempt!(to, fs::copy(from, to));
    ok(Value::Null)
}

fn rename(from: &str, to: &str) -> RResult<Value> {
    attempt!(from, fs::symlink_metadata(from));
    if is_dir(to) && !is_dir(from) {
        return fail("IsDirectory", format!("{to}: is a directory; give the full path to move the file to"));
    }
    if let Some(parent) = Path::new(to).parent().filter(|p| !p.as_os_str().is_empty()) {
        if !parent.is_dir() {
            return fail("NotFound", format!("{to}: the directory {} does not exist; file.make_dir creates it", parent.display()));
        }
    }
    match fs::rename(from, to) {
        Ok(()) => ok(Value::Null),
        // Renaming cannot cross from one drive to another; a file can be copied instead.
        Err(e) if e.kind() == io::ErrorKind::CrossesDevices && !is_dir(from) => {
            attempt!(to, fs::copy(from, to));
            attempt!(from, fs::remove_file(from));
            ok(Value::Null)
        }
        Err(e) => io_fail(from, e),
    }
}

fn remove(path: &str) -> RResult<Value> {
    if is_dir(path) {
        return fail("IsDirectory", format!("{path}: is a directory; file.remove_dir removes an empty one, file.remove_all one with files in it"));
    }
    attempt!(path, fs::remove_file(path));
    ok(Value::Null)
}

fn remove_dir(path: &str) -> RResult<Value> {
    match metadata(path) {
        Ok(m) if !m.is_dir() => return fail("NotDirectory", format!("{path}: is a file; file.remove removes files")),
        Ok(_) => {}
        Err(failure) => return failure,
    }
    attempt!(path, fs::remove_dir(path));
    ok(Value::Null)
}

/// Remove a directory and everything in it, except where that would destroy
/// far more than anyone means: a drive or filesystem root, the home directory,
/// or the working directory or one that contains it.
fn remove_all(path: &str) -> RResult<Value> {
    match fs::symlink_metadata(path) {
        Ok(m) if !m.is_dir() => return fail("NotDirectory", format!("{path}: is not a directory; file.remove removes files")),
        Ok(_) => {}
        Err(e) => return io_fail(path, e),
    }
    let target = attempt!(path, fs::canonicalize(path));
    let refuse = |why: &str| fail("Permission", format!("{path}: file.remove_all refuses to remove {why}"));
    if target.parent().is_none() { return refuse("the root of a drive or file system"); }
    let home = std::env::var_os("USERPROFILE").or_else(|| std::env::var_os("HOME")).and_then(|h| fs::canonicalize(h).ok());
    if home.is_some_and(|home| home == target) { return refuse("the home directory"); }
    if let Ok(cwd) = std::env::current_dir().and_then(fs::canonicalize) {
        if cwd.starts_with(&target) { return refuse("the working directory or a directory that contains it"); }
    }
    attempt!(path, fs::remove_dir_all(&target));
    ok(Value::Null)
}

// ----- places -----

/// `name` beside the running program's file.
fn here(name: &str) -> RResult<Value> {
    let program = program_path();
    let base = Path::new(&program).parent().filter(|_| !program.is_empty());
    Ok(match base {
        Some(dir) if !dir.as_os_str().is_empty() => display(&dir.join(name)),
        _ => Value::str(name),
    })
}

// ----- path text -----

fn parts(path: &str) -> Value {
    let mut parts: Vec<String> = Vec::new();
    for component in Path::new(path).components() {
        match component {
            // "C:" and the "\" after it are one part, "C:\", as in Python's pathlib.
            Component::Prefix(prefix) => parts.push(prefix.as_os_str().to_string_lossy().into_owned()),
            Component::RootDir => {
                let after_prefix = parts.len() == 1 && !parts[0].ends_with(std::path::MAIN_SEPARATOR);
                if after_prefix { parts[0].push(std::path::MAIN_SEPARATOR); } else { parts.push(std::path::MAIN_SEPARATOR.to_string()); }
            }
            other => parts.push(other.as_os_str().to_string_lossy().into_owned()),
        }
    }
    Value::array(parts.iter().map(|p| Value::str(p)).collect())
}

fn optional(text: Option<&std::ffi::OsStr>) -> Value {
    Value::str(&text.map(|t| t.to_string_lossy().into_owned()).unwrap_or_default())
}

// ----- CSV (RFC 4180) -----

/// Rows of fields, and the line each row starts on, for later errors.
/// `source` names where the text came from.
fn csv_parse(text: &str, source: &str) -> RResult<Value> {
    let text = text.strip_prefix('\u{feff}').unwrap_or(text);
    let place = |line: usize| if source.is_empty() { format!("CSV line {line}") } else { format!("{source}: CSV line {line}") };
    let (mut rows, mut starts) = (Vec::new(), Vec::new());
    let mut row: Vec<Value> = Vec::new();
    let mut field = String::new();
    // The line the current field's opening quote is on, or 0 when it is not quoted.
    let mut quoted_from = 0;
    let (mut line, mut row_line) = (1, 1);
    let mut chars = text.chars().peekable();
    // A blank line is no row at all; one quoted empty field is a row.
    let mut end_row = |row: &mut Vec<Value>, field: &mut String, quoted: bool, row_line: usize| {
        if row.is_empty() && field.is_empty() && !quoted { return; }
        row.push(Value::str(&std::mem::take(field)));
        rows.push(Value::array(std::mem::take(row)));
        starts.push(Value::Int(row_line as i64));
    };
    while let Some(c) = chars.next() {
        match c {
            '"' if field.is_empty() && quoted_from == 0 => {
                quoted_from = line;
                loop {
                    match chars.next() {
                        None => return err(ErrorKind::SyntaxError, format!("{}: this quoted field is never closed", place(quoted_from))),
                        Some('"') if chars.peek() == Some(&'"') => { chars.next(); field.push('"'); }
                        Some('"') => break,
                        Some('\n') => { line += 1; field.push('\n'); }
                        Some(other) => field.push(other),
                    }
                }
                if let Some(&other) = chars.peek().filter(|c| !matches!(c, ',' | '\n' | '\r')) {
                    return err(ErrorKind::SyntaxError, format!(
                        "{}: '{other}' after a closing quote; a quote inside a quoted field is written twice", place(line)
                    ));
                }
            }
            '"' => return err(ErrorKind::SyntaxError, format!(
                "{}: a quote inside an unquoted field; quote the whole field and write the quote twice", place(line)
            )),
            ',' => { row.push(Value::str(&std::mem::take(&mut field))); quoted_from = 0; }
            '\r' if chars.peek() == Some(&'\n') => {}
            '\n' | '\r' => {
                end_row(&mut row, &mut field, quoted_from != 0, row_line);
                quoted_from = 0;
                line += 1;
                row_line = line;
            }
            other => field.push(other),
        }
    }
    end_row(&mut row, &mut field, quoted_from != 0, row_line);
    Ok(Value::array(vec![Value::array(rows), Value::array(starts)]))
}

fn csv_field(value: &Value, out: &mut String) -> RResult<()> {
    let text = match value {
        Value::Str(text) => text.to_string(),
        Value::Null => String::new(),
        Value::Int(_) | Value::Long(_) | Value::Float(_) | Value::Bool(_) => to_display_string(value),
        other => {
            let kind = other.type_name();
            let article = if kind.starts_with(['a', 'e', 'i', 'o', 'u']) { "an" } else { "a" };
            return type_err(format!("a CSV field holds text or a number, not {article} {kind}"));
        }
    };
    if text.contains([',', '"', '\n', '\r']) {
        out.push('"');
        out.push_str(&text.replace('"', "\"\""));
        out.push('"');
    } else {
        out.push_str(&text);
    }
    Ok(())
}

fn csv_format(rows: &Value) -> RResult<Value> {
    let Value::Array(rows) = rows else { return type_err("csv.format expects an array of rows") };
    let mut out = String::new();
    for row in rows.items.borrow().iter() {
        let Value::Array(fields) = row else {
            return type_err(format!("each CSV row is an array of fields, not a {}", row.type_name()));
        };
        for (i, field) in fields.items.borrow().iter().enumerate() {
            if i > 0 { out.push(','); }
            csv_field(field, &mut out)?;
        }
        out.push('\n');
    }
    Ok(Value::str(&out))
}

// ----- the package -----

pub fn package() -> Value {
    NativePackage::new("rs.file")
        .function("read", 1, |a| read(text(&a[0], "file.read")?))
        .function("read_lines", 1, |a| read_lines(text(&a[0], "file.read_lines")?))
        .function("lines_chunk", 3, |a| {
            let count = match &a[2] { Value::Int(n) if *n > 0 => *n as usize, _ => return range_err("lines_chunk needs a positive count") };
            lines_chunk(text(&a[0], "file.each_line")?, &a[1], count)
        })
        .function("write", 2, |a| write(text(&a[0], "file.write")?, text(&a[1], "file.write")?.as_bytes()))
        .function("append", 2, |a| append(text(&a[0], "file.append")?, text(&a[1], "file.append")?))
        .function("create", 2, |a| create(text(&a[0], "file.create")?, text(&a[1], "file.create")?))
        .function("write_lines", 2, |a| write_lines(text(&a[0], "file.write_lines")?, &a[1]))
        .function("exists", 1, |a| {
            let path = text(&a[0], "file.exists")?;
            ok(Value::Bool(attempt!(path, Path::new(path).try_exists())))
        })
        .function("is_file", 1, |a| Ok(Value::Bool(Path::new(text(&a[0], "file.is_file")?).is_file())))
        .function("is_dir", 1, |a| Ok(Value::Bool(Path::new(text(&a[0], "file.is_dir")?).is_dir())))
        .function("size", 1, |a| size(text(&a[0], "file.size")?))
        .function("modified", 1, |a| modified(text(&a[0], "file.modified")?))
        .function("list", 1, |a| list(text(&a[0], "file.list")?))
        .function("walk", 1, |a| walk(text(&a[0], "file.walk")?))
        .function("make_dir", 1, |a| make_dir(text(&a[0], "file.make_dir")?))
        .function("copy", 2, |a| copy(text(&a[0], "file.copy")?, text(&a[1], "file.copy")?))
        .function("move", 2, |a| rename(text(&a[0], "file.move")?, text(&a[1], "file.move")?))
        .function("remove", 1, |a| remove(text(&a[0], "file.remove")?))
        .function("remove_dir", 1, |a| remove_dir(text(&a[0], "file.remove_dir")?))
        .function("remove_all", 1, |a| remove_all(text(&a[0], "file.remove_all")?))
        .function("here", 1, |a| here(text(&a[0], "file.here")?))
        .function("temp_dir", 0, |_| Ok(display(&std::env::temp_dir())))
        // Path text: std.file.path.
        .function("join", 2, |a| Ok(display(&Path::new(text(&a[0], "path.join")?).join(text(&a[1], "path.join")?))))
        .function("parent", 1, |a| Ok(optional(Path::new(text(&a[0], "path.parent")?).parent().map(Path::as_os_str))))
        .function("name", 1, |a| Ok(optional(Path::new(text(&a[0], "path.name")?).file_name())))
        .function("stem", 1, |a| Ok(optional(Path::new(text(&a[0], "path.stem")?).file_stem())))
        .function("extension", 1, |a| Ok(optional(Path::new(text(&a[0], "path.extension")?).extension())))
        .function("with_extension", 2, |a| {
            let path = text(&a[0], "path.with_extension")?;
            let extension = text(&a[1], "path.with_extension")?.trim_start_matches('.');
            if Path::new(path).file_name().is_none() {
                return range_err(format!("path.with_extension needs a path that ends in a name, not \"{path}\""));
            }
            Ok(display(&Path::new(path).with_extension(extension)))
        })
        .function("absolute", 1, |a| {
            let path = text(&a[0], "path.absolute")?;
            if path.is_empty() { return range_err("path.absolute needs a path, not an empty string"); }
            Ok(match std::path::absolute(path) { Ok(p) => display(&p), Err(e) => return err(ErrorKind::Error, format!("{path}: {e}")) })
        })
        .function("is_absolute", 1, |a| Ok(Value::Bool(Path::new(text(&a[0], "path.is_absolute")?).is_absolute())))
        .function("parts", 1, |a| Ok(parts(text(&a[0], "path.parts")?)))
        // CSV text: std.file.csv.
        .function("csv_parse", 2, |a| csv_parse(text(&a[0], "csv.parse")?, text(&a[1], "csv.parse")?))
        .function("csv_format", 1, |a| csv_format(&a[0]))
        .build()
}
