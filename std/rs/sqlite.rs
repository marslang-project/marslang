//! `takepkg rs.sqlite;` - SQLite databases for `std.sqlite`, by the rusqlite
//! crate with SQLite's own C source bundled in, so nothing needs installing.
//!
//! A native package cannot hold Marslang objects, so open connections live in
//! a thread-local registry keyed by a number that `std/sqlite.mars` keeps in
//! its `Database`. Numbers are never reused: a closed (or never opened) number
//! is simply not in the registry, which is how a closed database is noticed.
//! Each program runs on its own thread, and the registry is dropped when that
//! thread ends, which closes every connection left open.
//!
//! Every call that can fail for a database reason returns `[true, value]` or
//! `[false, kind, message]`, and the Marslang side raises the family `kind`
//! names, as `std/rs/file.rs` does for `std.file`. A parameter of a type that
//! SQLite cannot hold is the caller's mistake, so it raises TypeError or
//! RangeError here directly.
//!
//! Values cross as: int, longint, and bigint within 64 bits <-> INTEGER (read
//! back as int when it fits in 32 bits); float <-> REAL; string <-> TEXT;
//! null <-> NULL; bool -> 1 or 0; an array of byte ints <-> BLOB.

use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::path::Path;

use indexmap::IndexMap;
use num_traits::ToPrimitive;
use rusqlite::types::{Value as Sql, ValueRef};
use rusqlite::{params_from_iter, Connection, ErrorCode, OpenFlags};

use crate::value::*;

thread_local! {
    static CONNECTIONS: RefCell<HashMap<i64, Connection>> = RefCell::new(HashMap::new());
    static NEXT_ID: Cell<i64> = const { Cell::new(1) };
}

pub fn package() -> Value {
    NativePackage::new("rs.sqlite")
        .function("open", 1, |a| open(text(&a[0], "sqlite.open")?))
        .function("close", 1, |a| {
            // Dropping the connection closes it; a transaction still open is rolled back.
            let removed = CONNECTIONS.with(|all| all.borrow_mut().remove(&id(&a[0])));
            Ok(Value::Bool(removed.is_some()))
        })
        .function("is_open", 1, |a| Ok(Value::Bool(CONNECTIONS.with(|all| all.borrow().contains_key(&id(&a[0]))))))
        .function("run", 3, |a| with_connection(&a[0], |db| run(db, text(&a[1], "sqlite")?, &a[2])))
        .function("query", 4, |a| with_connection(&a[0], |db| query(db, text(&a[1], "sqlite")?, &a[2], limit(&a[3]))))
        .function("value", 3, |a| with_connection(&a[0], |db| value(db, text(&a[1], "sqlite")?, &a[2])))
        .function("script", 2, |a| with_connection(&a[0], |db| {
            let sql = text(&a[1], "sqlite")?;
            Ok(match db.execute_batch(sql) {
                Ok(()) => success(Value::Null),
                Err(error) => failure(&error, Some(sql)),
            })
        }))
        .function("last_id", 1, |a| with_connection(&a[0], |db| Ok(success(integer(db.last_insert_rowid())))))
        // Whether a transaction is open: false when SQLite is in autocommit mode.
        .function("in_transaction", 1, |a| with_connection(&a[0], |db| Ok(success(Value::Bool(!db.is_autocommit())))))
        .build()
}

fn text<'a>(value: &'a Value, what: &str) -> RResult<&'a str> {
    match value {
        Value::Str(text) => Ok(text),
        other => type_err(format!("{what} expects a string, got {}", other.type_name())),
    }
}

fn id(value: &Value) -> i64 {
    match value { Value::Int(n) | Value::Long(n) => *n, _ => -1 }
}

fn limit(value: &Value) -> Option<usize> {
    match value { Value::Int(n) if *n >= 0 => Some(*n as usize), _ => None }
}

fn success(value: Value) -> Value { Value::array(vec![Value::Bool(true), value]) }

fn fail(kind: &str, message: String) -> Value {
    Value::array(vec![Value::Bool(false), Value::str(kind), Value::str(&message)])
}

/// An SQLite INTEGER as a value: an int when it fits in 32 bits, else a longint.
fn integer(n: i64) -> Value {
    if i32::try_from(n).is_ok() { Value::Int(n) } else { Value::Long(n) }
}

/// Runs `work` with the open connection `handle` names, or reports that it is closed.
fn with_connection(handle: &Value, work: impl FnOnce(&Connection) -> RResult<Value>) -> RResult<Value> {
    CONNECTIONS.with(|all| match all.borrow().get(&id(handle)) {
        Some(db) => work(db),
        None => Ok(fail("Closed", "the database is closed".into())),
    })
}

// ----- opening -----

fn open(path: &str) -> RResult<Value> {
    if path.is_empty() {
        return range_err("sqlite.open needs a file path or \":memory:\", got an empty string");
    }
    let memory = path == ":memory:";
    if !memory {
        let target = Path::new(path);
        if target.is_dir() {
            return Ok(fail("IsDirectory", format!("{path}: is a directory, not a database file")));
        }
        if let Some(parent) = target.parent().filter(|parent| !parent.as_os_str().is_empty()) {
            if !parent.is_dir() {
                return Ok(fail("NotFound", format!("{path}: the directory {} does not exist", parent.display())));
            }
        }
    }
    // Plain flags: no URI interpretation, so "file:x?mode=ro" is just a file name.
    let flags = OpenFlags::SQLITE_OPEN_READ_WRITE | OpenFlags::SQLITE_OPEN_CREATE | OpenFlags::SQLITE_OPEN_NO_MUTEX;
    let db = match Connection::open_with_flags(path, flags) {
        Ok(db) => db,
        Err(error) => return Ok(fail("Sql", format!("{path}: cannot open the database: {}", message(&error)))),
    };
    // SQLite leaves foreign keys unchecked unless asked, per connection.
    if let Err(error) = db.execute_batch("PRAGMA foreign_keys = ON") {
        return Ok(fail("Sql", format!("{path}: {}", message(&error))));
    }
    let number = NEXT_ID.with(|next| next.replace(next.get() + 1));
    CONNECTIONS.with(|all| all.borrow_mut().insert(number, db));
    Ok(success(Value::Int(number)))
}

// ----- running -----

/// A statement with no result rows; the number of rows it changed.
fn run(db: &Connection, sql: &str, params: &Value) -> RResult<Value> {
    let bound = parameters(params)?;
    let mut statement = match db.prepare(sql) {
        Ok(statement) => statement,
        Err(error) => return Ok(failure(&error, Some(sql))),
    };
    if let Some(mismatch) = count_mismatch(&statement, bound.len(), sql) { return Ok(mismatch); }
    let before = db.total_changes();
    match statement.execute(params_from_iter(&bound)) {
        // changes() keeps the count of the last INSERT, UPDATE, or DELETE through a CREATE
        // TABLE, so ask only when something changed.
        Ok(_) => Ok(success(integer(if db.total_changes() == before { 0 } else { db.changes() as i64 }))),
        Err(error) => Ok(failure(&error, Some(sql))),
    }
}

/// The rows of a statement, as an array of maps, at most `limit` of them.
fn query(db: &Connection, sql: &str, params: &Value, limit: Option<usize>) -> RResult<Value> {
    let bound = parameters(params)?;
    let mut statement = match db.prepare(sql) {
        Ok(statement) => statement,
        Err(error) => return Ok(failure(&error, Some(sql))),
    };
    if let Some(mismatch) = count_mismatch(&statement, bound.len(), sql) { return Ok(mismatch); }
    let names: Vec<Value> = statement.column_names().into_iter().map(Value::str).collect();
    let mut rows = match statement.query(params_from_iter(&bound)) {
        Ok(rows) => rows,
        Err(error) => return Ok(failure(&error, Some(sql))),
    };
    let mut found = Vec::new();
    while limit.is_none_or(|most| found.len() < most) {
        let row = match rows.next() {
            Ok(Some(row)) => row,
            Ok(None) => break,
            Err(error) => return Ok(failure(&error, Some(sql))),
        };
        // A name that repeats (SELECT a.id, b.id) keeps its first place and its last value.
        let mut items = IndexMap::new();
        for (at, name) in names.iter().enumerate() {
            let cell = match row.get_ref(at) {
                Ok(cell) => cell,
                Err(error) => return Ok(failure(&error, Some(sql))),
            };
            match cell_value(cell) {
                Ok(value) => { items.insert(Key::of(name), (name.clone(), value)); }
                Err(problem) => {
                    return Ok(fail("Sql", format!("column {} holds {problem}", crate::bigint::quote(&name_text(name)))));
                }
            }
        }
        found.push(Value::new_map(items, Meta::default()));
    }
    Ok(success(Value::array(found)))
}

/// The first column of the first row, or null when there is no row.
fn value(db: &Connection, sql: &str, params: &Value) -> RResult<Value> {
    let bound = parameters(params)?;
    let mut statement = match db.prepare(sql) {
        Ok(statement) => statement,
        Err(error) => return Ok(failure(&error, Some(sql))),
    };
    if let Some(mismatch) = count_mismatch(&statement, bound.len(), sql) { return Ok(mismatch); }
    if statement.column_count() == 0 {
        return Ok(fail("Sql", format!("the statement returns no columns, so there is no value to read{}", context(sql))));
    }
    let mut rows = match statement.query(params_from_iter(&bound)) {
        Ok(rows) => rows,
        Err(error) => return Ok(failure(&error, Some(sql))),
    };
    match rows.next() {
        Ok(Some(row)) => match row.get_ref(0).map(cell_value) {
            Ok(Ok(value)) => Ok(success(value)),
            Ok(Err(problem)) => Ok(fail("Sql", format!("the first column holds {problem}"))),
            Err(error) => Ok(failure(&error, Some(sql))),
        },
        Ok(None) => Ok(success(Value::Null)),
        Err(error) => Ok(failure(&error, Some(sql))),
    }
}

/// A failure when the statement's placeholders and the parameters differ in number. Checked here
/// because rusqlite reports the position where binding stopped, not how many were given.
fn count_mismatch(statement: &rusqlite::Statement, given: usize, sql: &str) -> Option<Value> {
    let expected = statement.parameter_count();
    if expected == given { return None; }
    let text = message(&rusqlite::Error::InvalidParameterCount(given, expected));
    Some(fail("Sql", format!("{text}{}", context(sql))))
}

fn name_text(name: &Value) -> String {
    match name { Value::Str(text) => text.to_string(), _ => String::new() }
}

// ----- values -----

/// A result cell as a Marslang value, or what is wrong with it.
fn cell_value(cell: ValueRef) -> Result<Value, &'static str> {
    Ok(match cell {
        ValueRef::Null => Value::Null,
        ValueRef::Integer(n) => integer(n),
        ValueRef::Real(x) => Value::Float(x),
        ValueRef::Text(bytes) => match std::str::from_utf8(bytes) {
            Ok(text) => Value::str(text),
            Err(_) => return Err("text that is not valid UTF-8"),
        },
        ValueRef::Blob(bytes) => Value::array(bytes.iter().map(|byte| Value::Int(i64::from(*byte))).collect()),
    })
}

/// The `?` parameters as SQLite values; a type SQLite cannot hold is the
/// caller's mistake and names its position, counting from 1.
fn parameters(params: &Value) -> RResult<Vec<Sql>> {
    let Value::Array(items) = params else {
        return type_err(format!("SQL parameters must be an array, got {}", params.type_name()));
    };
    let items = items.items.borrow();
    let mut bound = Vec::with_capacity(items.len());
    for (at, item) in items.iter().enumerate() {
        let position = at + 1;
        bound.push(match item {
            Value::Null => Sql::Null,
            Value::Bool(flag) => Sql::Integer(i64::from(*flag)),
            Value::Int(n) | Value::Long(n) => Sql::Integer(*n),
            Value::Float(x) => Sql::Real(*x),
            Value::Str(text) => Sql::Text(text.to_string()),
            Value::Big(big) => match big.to_i64() {
                Some(n) => Sql::Integer(n),
                None => return range_err(format!(
                    "SQL parameter {position} is the bigint {}, which does not fit in 64 bits, the size of an SQLite INTEGER; pass string(n) to store it as text",
                    crate::bigint::quote(&big.to_string()))),
            },
            Value::Array(bytes) => Sql::Blob(blob(&bytes.items.borrow(), position)?),
            other => return type_err(format!(
                "SQL parameter {position} is a {}, which SQLite cannot store; use int, longint, bigint, float, string, bool, null, or an array of byte ints (0 to 255) for a blob",
                other.type_name())),
        });
    }
    Ok(bound)
}

fn blob(items: &[Value], position: usize) -> RResult<Vec<u8>> {
    let mut bytes = Vec::with_capacity(items.len());
    for (at, item) in items.iter().enumerate() {
        match item {
            Value::Int(n) | Value::Long(n) if (0..=255).contains(n) => bytes.push(*n as u8),
            Value::Int(n) | Value::Long(n) => return type_err(format!(
                "SQL parameter {position} is an array, stored as a blob, which holds ints from 0 to 255, but item {} is {n}", at + 1)),
            other => return type_err(format!(
                "SQL parameter {position} is an array, stored as a blob, which holds ints from 0 to 255, but item {} is a {}",
                at + 1, other.type_name())),
        }
    }
    Ok(bytes)
}

// ----- errors -----

/// SQLite's own words for what went wrong, or ours for the mistakes rusqlite checks.
fn message(error: &rusqlite::Error) -> String {
    use rusqlite::Error::*;
    match error {
        SqliteFailure(_, Some(text)) => text.clone(),
        // A syntax error: rusqlite's own text repeats the whole SQL, which the caller shortens below.
        SqlInputError { msg, offset, .. } => format!("{msg} (at offset {offset})"),
        SqliteFailure(code, None) => code.to_string(),
        InvalidParameterCount(given, expected) => format!(
            "the SQL has {expected} ? placeholder{} but {given} parameter{} given",
            if *expected == 1 { "" } else { "s" },
            if *given == 1 { " was" } else { "s were" }),
        MultipleStatement => "the SQL holds more than one statement; run, query, query_one, and value take one, so use script for several".into(),
        ExecuteReturnedResults => "the statement returns rows; run is for statements that do not, so use query".into(),
        other => other.to_string(),
    }
}

/// `[false, "Sql", message]`; the SQL is added, shortened, unless a constraint
/// failed, whose message already names the table and column.
fn failure(error: &rusqlite::Error, sql: Option<&str>) -> Value {
    let constraint = matches!(error, rusqlite::Error::SqliteFailure(code, _) if code.code == ErrorCode::ConstraintViolation);
    let tail = match sql { Some(sql) if !constraint => context(sql), _ => String::new() };
    fail("Sql", format!("{}{tail}", message(error)))
}

/// ` [SQL: ...]` with the statement on one line and cut to 80 characters.
fn context(sql: &str) -> String {
    let flat = sql.split_whitespace().collect::<Vec<_>>().join(" ");
    let mut chars = flat.chars();
    let head: String = chars.by_ref().take(80).collect();
    if chars.next().is_some() { format!(" [SQL: {head}...]") } else { format!(" [SQL: {head}]") }
}
