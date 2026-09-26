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

/// A name no one can guess: SipHash keyed from the operating system's random
/// source, so another user cannot plant a file or link where it will go.
fn unguessable() -> String {
    use std::hash::{BuildHasher, Hasher};
    let mut hasher = std::collections::hash_map::RandomState::new().build_hasher();
    hasher.write_u64(TEMPORARY.fetch_add(1, Ordering::Relaxed));
    hasher.write_u32(std::process::id());
    format!("{:016x}", hasher.finish())
}

/// Replace the file all at once: write a temporary file beside it, flush it
/// to disk, then rename it over the old one, so a crash or a full disk leaves
/// either the old contents or the new, never half of either.
///
/// The temporary file is created exclusively under an unguessable name, so it
/// can never be a link someone planted, and it takes the old file's
/// permissions (and owner, where allowed) before replacing it: a private file
/// stays private. Writing through a link updates the file it points to.
fn write(path: &str, contents: &[u8]) -> RResult<Value> {
    if let Some(failure) = check_parent(path) { return failure; }
    let mut target = PathBuf::from(path);
    if fs::symlink_metadata(&target).is_ok_and(|m| m.file_type().is_symlink()) {
        if let Ok(real) = fs::canonicalize(&target) { target = real; }
    }
    let existing = fs::metadata(&target).ok().filter(|m| m.is_file());
    let name = target.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    let mut temporary = PathBuf::new();
    let written = (|| {
        let mut attempts = 0;
        let mut file = loop {
            temporary = target.with_file_name(format!(".{name}.{}.tmp", unguessable()));
            let mut options = fs::OpenOptions::new();
            options.write(true).create_new(true);
            // Private while it is written; it gets its final permissions below.
            #[cfg(unix)]
            if existing.is_some() { std::os::unix::fs::OpenOptionsExt::mode(&mut options, 0o600); }
            match options.open(&temporary) {
                Ok(file) => break file,
                Err(e) if e.kind() == io::ErrorKind::AlreadyExists && attempts < 8 => attempts += 1,
                Err(e) => { temporary = PathBuf::new(); return Err(e); }
            }
        };
        file.write_all(contents)?;
        file.sync_all()?;
        drop(file);
        if let Some(old) = &existing {
            fs::set_permissions(&temporary, old.permissions())?;
            #[cfg(unix)]
            {
                use std::os::unix::fs::MetadataExt;
                // Keeps the owner when the writer may; otherwise the writer owns it.
                let _ = std::os::unix::fs::chown(&temporary, Some(old.uid()), Some(old.gid()));
            }
        }
        fs::rename(&temporary, &target)
    })();
    if let Err(error) = written {
        if !temporary.as_os_str().is_empty() { let _ = fs::remove_file(&temporary); }
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

// ----- more operations -----

/// Copy a directory and everything in it. `to` must not exist yet; if the copy
/// fails partway, what was made is removed again, so there is never half a copy.
fn copy_all(from: &str, to: &str) -> RResult<Value> {
    match metadata(from) {
        Ok(m) if !m.is_dir() => return fail("NotDirectory", format!("{from}: is a file; file.copy copies files")),
        Ok(_) => {}
        Err(failure) => return failure,
    }
    if Path::new(to).exists() {
        return fail("Exists", format!("{to}: already exists; file.copy_all makes a new directory"));
    }
    if let Some(failure) = check_parent(to) { return failure; }
    let inside = std::path::absolute(to).ok().zip(std::path::absolute(from).ok())
        .is_some_and(|(to, from)| lexical(&to).starts_with(lexical(&from)));
    if inside { return fail("File", format!("{to}: is inside {from}, so copying it there would never end")); }
    let copied = copy_tree(Path::new(from), Path::new(to));
    if copied.is_err() { let _ = fs::remove_dir_all(to); }
    match copied { Ok(()) => ok(Value::Null), Err((kind, message)) => fail(kind, message) }
}

fn copy_tree(from: &Path, to: &Path) -> Result<(), (&'static str, String)> {
    let shown = |p: &Path| p.to_string_lossy().into_owned();
    let io = |p: &Path, e: io::Error| ("File", format!("{}: {e}", shown(p)));
    fs::create_dir(to).map_err(|e| io(to, e))?;
    for entry in fs::read_dir(from).map_err(|e| io(from, e))? {
        let entry = entry.map_err(|e| io(from, e))?;
        let (source, target) = (entry.path(), to.join(entry.file_name()));
        let kind = entry.file_type().map_err(|e| io(&source, e))?;
        if kind.is_dir() {
            copy_tree(&source, &target)?;
        } else if kind.is_symlink() && fs::metadata(&source).is_ok_and(|m| m.is_dir()) {
            return Err(("File", format!("{}: is a link to a directory, which file.copy_all does not follow", shown(&source))));
        } else {
            fs::copy(&source, &target).map_err(|e| io(&source, e))?;
        }
    }
    Ok(())
}

/// Facts about a file or directory, as one map.
fn info(path: &str) -> RResult<Value> {
    let link = attempt!(path, fs::symlink_metadata(path)).file_type().is_symlink();
    let m = match fs::metadata(path) {
        Ok(m) => m,
        Err(_) if link => return fail("NotFound", format!("{path}: a link to something that is not there")),
        Err(e) => return io_fail(path, e),
    };
    let seconds = |time: io::Result<std::time::SystemTime>| match time {
        Ok(t) => Value::Float(match t.duration_since(std::time::UNIX_EPOCH) {
            Ok(after) => after.as_secs_f64(),
            Err(before) => -before.duration().as_secs_f64(),
        }),
        Err(_) => Value::Null,
    };
    let kind = if m.is_file() { "file" } else if m.is_dir() { "directory" } else { "other" };
    let entries = [
        ("kind", Value::str(kind)),
        ("link", Value::Bool(link)),
        ("size", if m.is_dir() { Value::Null } else { number(m.len()) }),
        ("modified", seconds(m.modified())),
        ("created", seconds(m.created())),
        ("readonly", Value::Bool(m.permissions().readonly())),
    ];
    let mut items = indexmap::IndexMap::new();
    for (key, value) in entries {
        let key = Value::str(key);
        items.insert(Key::of(&key), (key, value));
    }
    ok(Value::new_map(items, Meta::default()))
}

/// Whether two paths name one file or directory, once links, `.`, and `..`
/// are resolved.
fn same(a: &str, b: &str) -> RResult<Value> {
    let a_real = attempt!(a, fs::canonicalize(a));
    let b_real = attempt!(b, fs::canonicalize(b));
    ok(Value::Bool(a_real == b_real))
}

/// A new, empty directory under the system's temporary directory, under an
/// unguessable name, and private to this user where the system allows.
fn make_temp_dir() -> RResult<Value> {
    let base = std::env::temp_dir();
    let mut attempts = 0;
    loop {
        let dir = base.join(format!("marslang-{}", unguessable()));
        let shown = dir.to_string_lossy().into_owned();
        let builder = {
            #[allow(unused_mut)]
            let mut builder = fs::DirBuilder::new();
            #[cfg(unix)]
            std::os::unix::fs::DirBuilderExt::mode(&mut builder, 0o700);
            builder
        };
        match builder.create(&dir) {
            Ok(()) => return ok(Value::str(&shown)),
            Err(e) if e.kind() == io::ErrorKind::AlreadyExists && attempts < 8 => attempts += 1,
            Err(e) => return io_fail(&shown, e),
        }
    }
}

// ----- matching names against patterns -----

/// Whether a name matches one part of a pattern: `*` is any run of
/// characters, `?` one character, and `[abc]`, `[a-z]`, `[!abc]` one of a set.
fn name_matches(pattern: &[char], name: &[char]) -> bool {
    let fold = |c: char| if cfg!(windows) { c.to_lowercase().next().unwrap_or(c) } else { c };
    match pattern.first() {
        None => name.is_empty(),
        Some('*') => (0..=name.len()).any(|skip| name_matches(&pattern[1..], &name[skip..])),
        Some('?') => !name.is_empty() && name_matches(&pattern[1..], &name[1..]),
        Some('[') if pattern.contains(&']') => {
            let Some(&c) = name.first() else { return false };
            let close = pattern.iter().skip(2).position(|&p| p == ']').map(|at| at + 2).unwrap_or(1);
            let mut set = &pattern[1..close];
            let negate = set.first() == Some(&'!');
            if negate { set = &set[1..]; }
            let mut found = false;
            let mut i = 0;
            while i < set.len() {
                if i + 2 < set.len() && set[i + 1] == '-' {
                    if (fold(set[i])..=fold(set[i + 2])).contains(&fold(c)) { found = true; }
                    i += 3;
                } else {
                    if fold(set[i]) == fold(c) { found = true; }
                    i += 1;
                }
            }
            found != negate && name_matches(&pattern[close + 1..], &name[1..])
        }
        Some(&p) => name.first().is_some_and(|&c| fold(c) == fold(p)) && name_matches(&pattern[1..], &name[1..]),
    }
}

/// A name starting with a dot is matched only by a pattern part that starts
/// with one, as in a shell, so `*` does not reach into `.git`.
fn part_matches(pattern: &str, name: &str) -> bool {
    if name.starts_with('.') && !pattern.starts_with('.') { return false; }
    name_matches(&pattern.chars().collect::<Vec<_>>(), &name.chars().collect::<Vec<_>>())
}

/// Whether the path parts match the pattern parts, where `**` is any number
/// of directories, none included.
fn parts_match(pattern: &[String], path: &[String]) -> bool {
    match pattern.first().map(String::as_str) {
        None => path.is_empty(),
        Some("**") => parts_match(&pattern[1..], path)
            || (!path.is_empty() && !path[0].starts_with('.') && parts_match(pattern, &path[1..])),
        Some(part) => !path.is_empty() && part_matches(part, &path[0]) && parts_match(&pattern[1..], &path[1..]),
    }
}

/// Whether anything below the path parts could still match, so a directory
/// that cannot is not searched.
fn could_match_below(pattern: &[String], path: &[String]) -> bool {
    if path.is_empty() { return true; }
    match pattern.first().map(String::as_str) {
        None => false,
        Some("**") => could_match_below(&pattern[1..], path) || (!path[0].starts_with('.') && could_match_below(pattern, &path[1..])),
        Some(part) => part_matches(part, &path[0]) && could_match_below(&pattern[1..], &path[1..]),
    }
}

fn matching(dir: &str, pattern: &str) -> RResult<Value> {
    let separators: &[char] = if cfg!(windows) { &['/', '\\'] } else { &['/'] };
    if pattern.is_empty() { return range_err("file.matching needs a pattern, such as \"*.txt\""); }
    if Path::new(pattern).is_absolute() || pattern.starts_with(separators) {
        return range_err(format!("file.matching takes a pattern relative to the directory, not \"{pattern}\""));
    }
    let parts: Vec<String> = pattern.split(separators).filter(|p| !p.is_empty() && *p != ".").map(String::from).collect();
    if parts.iter().any(|p| p == "..") {
        return range_err(format!("file.matching patterns stay inside the directory; \"{pattern}\" goes above it with .."));
    }
    match metadata(dir) {
        Ok(m) if !m.is_dir() => return fail("NotDirectory", format!("{dir}: is a file, not a directory")),
        Ok(_) => {}
        Err(failure) => return failure,
    }
    let mut found = Vec::new();
    let mut pending: Vec<(PathBuf, Vec<String>)> = vec![(PathBuf::from(dir), Vec::new())];
    while let Some((at, relative)) = pending.pop() {
        let shown = at.to_string_lossy().into_owned();
        for entry in attempt!(&shown, fs::read_dir(&at)) {
            let entry = attempt!(&shown, entry);
            let mut here = relative.clone();
            here.push(entry.file_name().to_string_lossy().into_owned());
            if parts_match(&parts, &here) { found.push(entry.path()); }
            // Links to directories are not entered, as in walk.
            if attempt!(&shown, entry.file_type()).is_dir() && could_match_below(&parts, &here) {
                pending.push((entry.path(), here));
            }
        }
    }
    found.sort();
    ok(Value::array(found.iter().map(|p| display(p)).collect()))
}

// ----- path text -----

/// `.` removed and `..` taken back against the part before it, without
/// looking at the disk: `a/./b/../c` is `a/c`. `..` at a root stays at the root.
fn lexical(path: &Path) -> PathBuf {
    let mut out: Vec<Component> = Vec::new();
    for component in path.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => match out.last() {
                Some(Component::Normal(_)) => { out.pop(); }
                Some(Component::RootDir | Component::Prefix(_)) => {}
                _ => out.push(component),
            },
            other => out.push(other),
        }
    }
    let joined: PathBuf = out.iter().collect();
    if joined.as_os_str().is_empty() { PathBuf::from(".") } else { joined }
}

fn same_part(a: &Component, b: &Component) -> bool {
    if cfg!(windows) {
        a.as_os_str().to_string_lossy().to_lowercase() == b.as_os_str().to_string_lossy().to_lowercase()
    } else {
        a == b
    }
}

/// The path that leads from `base` to `path`: `relative("docs/api/x.md", "docs")`
/// is `api/x.md`, and from `docs/api` to `docs/guide` is `../guide`.
fn relative(path: &str, base: &str) -> RResult<Value> {
    let (mut to, mut from) = (lexical(Path::new(path)), lexical(Path::new(base)));
    if to.is_absolute() != from.is_absolute() {
        let absolute = |p: &Path| std::path::absolute(p).map(|p| lexical(&p));
        match (absolute(&to), absolute(&from)) {
            (Ok(a), Ok(b)) => { to = a; from = b; }
            _ => return err(ErrorKind::Error, "the working directory cannot be read"),
        }
    }
    let to_parts: Vec<Component> = to.components().filter(|c| *c != Component::CurDir).collect();
    let from_parts: Vec<Component> = from.components().filter(|c| *c != Component::CurDir).collect();
    let rooted = |parts: &[Component]| parts.iter().take_while(|c| matches!(c, Component::Prefix(_) | Component::RootDir)).count();
    let (to_root, from_root) = (rooted(&to_parts), rooted(&from_parts));
    if to_root != from_root || !to_parts[..to_root].iter().zip(&from_parts[..from_root]).all(|(a, b)| same_part(a, b)) {
        return range_err(format!("\"{path}\" and \"{base}\" start from different roots or drives, so no relative path joins them"));
    }
    let common = to_parts.iter().zip(&from_parts).take_while(|(a, b)| same_part(a, b)).count();
    if from_parts[common..].iter().any(|c| *c == Component::ParentDir) {
        return range_err(format!("\"{base}\" goes above where it starts with .., so the way back to \"{path}\" is unknown"));
    }
    let mut result = PathBuf::new();
    for _ in common..from_parts.len() { result.push(".."); }
    for part in &to_parts[common..] { result.push(part.as_os_str()); }
    Ok(if result.as_os_str().is_empty() { Value::str(".") } else { display(&result) })
}


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
fn csv_parse(text: &str, source: &str, separator: char) -> RResult<Value> {
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
                if let Some(&other) = chars.peek().filter(|&&c| c != separator && c != '\n' && c != '\r') {
                    return err(ErrorKind::SyntaxError, format!(
                        "{}: '{other}' after a closing quote; a quote inside a quoted field is written twice", place(line)
                    ));
                }
            }
            '"' => return err(ErrorKind::SyntaxError, format!(
                "{}: a quote inside an unquoted field; quote the whole field and write the quote twice", place(line)
            )),
            c if c == separator => { row.push(Value::str(&std::mem::take(&mut field))); quoted_from = 0; }
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

fn csv_field(value: &Value, out: &mut String, separator: char) -> RResult<()> {
    let text = match value {
        Value::Str(text) => text.to_string(),
        Value::Null => String::new(),
        Value::Int(_) | Value::Long(_) | Value::Float(_) | Value::Bool(_) => to_display_string(value),
        Value::Date(_) | Value::DateTime(_) | Value::Duration(_) => to_display_string(value),
        other => {
            let kind = other.type_name();
            let article = if kind.starts_with(['a', 'e', 'i', 'o', 'u']) { "an" } else { "a" };
            return type_err(format!("a CSV field holds text or a number, not {article} {kind}"));
        }
    };
    if text.contains([separator, '"', '\n', '\r']) {
        out.push('"');
        out.push_str(&text.replace('"', "\"\""));
        out.push('"');
    } else {
        out.push_str(&text);
    }
    Ok(())
}

fn csv_format(rows: &Value, separator: char) -> RResult<Value> {
    let Value::Array(rows) = rows else { return type_err("csv.format expects an array of rows") };
    let mut out = String::new();
    for row in rows.items.borrow().iter() {
        let Value::Array(fields) = row else {
            return type_err(format!("each CSV row is an array of fields, not a {}", row.type_name()));
        };
        for (i, field) in fields.items.borrow().iter().enumerate() {
            if i > 0 { out.push(separator); }
            csv_field(field, &mut out, separator)?;
        }
        out.push('\n');
    }
    Ok(Value::str(&out))
}

/// The one character that separates CSV fields.
fn separator(value: &Value) -> RResult<char> {
    let text = text(value, "csv.with_separator")?;
    let mut chars = text.chars();
    match (chars.next(), chars.next()) {
        (Some(c), None) if !matches!(c, '"' | '\n' | '\r') => Ok(c),
        (Some(_), None) => range_err("a CSV separator cannot be a quote or a line break"),
        _ => range_err(format!("a CSV separator is one character, such as \";\" or \"\\t\", not \"{text}\"")),
    }
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
        .function("csv_parse", 3, |a| csv_parse(text(&a[0], "csv.parse")?, text(&a[1], "csv.parse")?, separator(&a[2])?))
        .function("csv_format", 2, |a| csv_format(&a[0], separator(&a[1])?))
        .function("csv_separator", 1, |a| separator(&a[0]).map(|c| Value::str(c.encode_utf8(&mut [0; 4]))))
        // More operations.
        .function("copy_all", 2, |a| copy_all(text(&a[0], "file.copy_all")?, text(&a[1], "file.copy_all")?))
        .function("info", 1, |a| info(text(&a[0], "file.info")?))
        .function("same", 2, |a| same(text(&a[0], "file.same")?, text(&a[1], "file.same")?))
        .function("make_temp_dir", 0, |_| make_temp_dir())
        .function("matching", 2, |a| matching(text(&a[0], "file.matching")?, text(&a[1], "file.matching")?))
        .function("normalize", 1, |a| Ok(display(&lexical(Path::new(text(&a[0], "path.normalize")?)))))
        .function("relative", 2, |a| relative(text(&a[0], "path.relative")?, text(&a[1], "path.relative")?))
        .build()
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt;

    #[test]
    fn temporary_directories_are_private() {
        let Value::Array(result) = make_temp_dir().expect("made") else { panic!("not a result") };
        let Value::Str(dir) = result.items.borrow()[1].clone() else { panic!("no path") };
        let mode = fs::metadata(&*dir).expect("exists").permissions().mode() & 0o777;
        fs::remove_dir(&*dir).expect("removed");
        assert_eq!(mode, 0o700, "{dir}");
    }
}
