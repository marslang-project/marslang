//! std.sqlite: statements and queries in memory and in files, every kind of
//! value both ways, the errors and their messages, transactions and their
//! nesting, closed databases, and that connections left open are closed when
//! the program ends.

use std::path::PathBuf;

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

/// A directory in the temporary directory, removed with everything in it when dropped.
struct TempDir(PathBuf);

impl TempDir {
    fn new(name: &str) -> TempDir {
        let path = std::env::temp_dir().join(format!("marslang-std-sqlite-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&path);
        std::fs::create_dir_all(&path).expect("create temp dir");
        TempDir(path)
    }

    fn path(&self, name: &str) -> PathBuf { self.0.join(name) }

    /// A path in the directory as Marslang source: raw, so Windows backslashes stay as they are.
    fn literal(&self, name: &str) -> String {
        let text = self.path(name).display().to_string();
        assert!(!text.contains('"'), "temp path with a quote: {text}");
        format!("r\"{text}\"")
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

#[test]
fn create_insert_query_update_and_delete() {
    executes(r#"
        takepkg std.sqlite;
        func m{
            db = sqlite.open(":memory:");
            out(db.run("CREATE TABLE users(id INTEGER PRIMARY KEY, name TEXT, age INTEGER)", arr()));
            out(db.run("INSERT INTO users(name, age) VALUES (?, ?)", arr("Ada", 36)));
            out(db.run("INSERT INTO users(name, age) VALUES (?, ?)", arr("Alan", 41)));
            out(db.run("INSERT INTO users(name, age) VALUES (?, ?)", arr("Grace", 85)));
            out(db.query("SELECT id, name, age FROM users ORDER BY id", arr()));
            out(db.run("UPDATE users SET age = age + 1 WHERE age < ?", arr(50)));
            out(db.query("SELECT name, age FROM users WHERE age > ? ORDER BY age", arr(40)));
            out(db.run("DELETE FROM users WHERE name = ?", arr("Alan")));
            out(db.run("DELETE FROM users WHERE name = ?", arr("nobody")));
            out(db.value("SELECT count(*) FROM users", arr()));
            out(db.query("SELECT * FROM users WHERE age > ?", arr(1000)));
            db.close();
        }"#,
        "0\n1\n1\n1\n\
         [{\"id\": 1, \"name\": \"Ada\", \"age\": 36}, {\"id\": 2, \"name\": \"Alan\", \"age\": 41}, {\"id\": 3, \"name\": \"Grace\", \"age\": 85}]\n\
         2\n\
         [{\"name\": \"Alan\", \"age\": 42}, {\"name\": \"Grace\", \"age\": 85}]\n\
         1\n0\n2\n[]\n");
}

#[test]
fn run_reports_zero_for_statements_after_an_insert_that_change_no_rows() {
    // SQLite keeps the count of the last INSERT through a CREATE TABLE; run must not.
    executes(r#"
        takepkg std.sqlite;
        func m{
            db = sqlite.open(":memory:");
            db.run("CREATE TABLE a(x)", arr());
            out(db.run("INSERT INTO a VALUES (1)", arr()));
            out(db.run("INSERT INTO a SELECT x + 1 FROM a", arr()));
            out(db.run("CREATE TABLE b(y)", arr()));
            out(db.run("DROP TABLE b", arr()));
            out(db.run("UPDATE a SET x = 0 WHERE x > 100", arr()));
        }"#,
        "1\n1\n0\n0\n0\n");
}

#[test]
fn rows_keep_column_order_and_a_repeated_name_keeps_its_last_value() {
    executes(r#"
        takepkg std.sqlite;
        func m{
            db = sqlite.open(":memory:");
            row = db.query_one("SELECT 3 AS z, 1 AS a, 2 AS m", arr());
            out(row.keys());
            out(db.query_one("SELECT 1 AS x, 2 AS y, 3 AS x", arr()));
            out(db.query("SELECT 1 AS n UNION ALL SELECT 2 UNION ALL SELECT 3", arr()));
        }"#,
        "[\"z\", \"a\", \"m\"]\n{\"x\": 3, \"y\": 2}\n[{\"n\": 1}, {\"n\": 2}, {\"n\": 3}]\n");
}

#[test]
fn a_file_database_is_created_and_keeps_its_data_between_connections() {
    let dir = TempDir::new("file");
    let path = dir.literal("notes.db");
    executes(&format!(r#"
        takepkg std.sqlite;
        takepkg std.file;
        func m{{
            out(file.exists({path}));
            db = sqlite.open({path});
            out(file.exists({path}));
            db.script("CREATE TABLE notes(id INTEGER PRIMARY KEY, body TEXT); INSERT INTO notes(body) VALUES ('first');");
            db.run("INSERT INTO notes(body) VALUES (?)", arr("second"));
            db.close();

            again = sqlite.open({path});
            out(again.query("SELECT body FROM notes ORDER BY id", arr()));
            again.close();
        }}"#),
        "false\ntrue\n[{\"body\": \"first\"}, {\"body\": \"second\"}]\n");
    assert!(dir.path("notes.db").is_file());
}

#[test]
fn every_kind_of_value_round_trips() {
    executes(r#"
        takepkg std.sqlite;
        takepkg std.types;
        func m{
            db = sqlite.open(":memory:");
            db.run("CREATE TABLE v(x)", arr());
            values = arr(0, -7, 2147483647, -2147483648, 2147483648, 5000000000, longint("9223372036854775807"),
                longint("-9223372036854775808"), 2.5, -0.125, 1e300, "text", "", "héllo 👨‍👩‍👧‍👦 日本", null);
            for (v, values){ db.run("INSERT INTO v VALUES (?)", arr(v)); }
            for (row, db.query("SELECT x FROM v ORDER BY rowid", arr())){
                x = row.get("x");
                out(types.kind(x), x);
            }
        }"#,
        "int 0\nint -7\nint 2147483647\nint -2147483648\nlongint 2147483648\nlongint 5000000000\n\
         longint 9223372036854775807\nlongint -9223372036854775808\nfloat 2.5\nfloat -0.125\nfloat 1e+300\n\
         string text\nstring \nstring héllo 👨‍👩‍👧‍👦 日本\nnull null\n");
}

#[test]
fn a_whole_float_stays_a_float_and_text_that_looks_like_a_number_stays_text() {
    executes(r#"
        takepkg std.sqlite;
        takepkg std.types;
        func m{
            db = sqlite.open(":memory:");
            db.run("CREATE TABLE v(x BLOB)", arr());
            db.run("INSERT INTO v VALUES (?), (?)", arr(3.0, "42"));
            for (row, db.query("SELECT x, typeof(x) AS t FROM v ORDER BY rowid", arr())){
                out(types.kind(row.get("x")), row.get("t"));
            }
        }"#,
        "float real\nstring text\n");
}

#[test]
fn bools_are_stored_as_one_and_zero() {
    executes(r#"
        takepkg std.sqlite;
        func m{
            db = sqlite.open(":memory:");
            db.run("CREATE TABLE flags(on_ INTEGER)", arr());
            db.run("INSERT INTO flags VALUES (?), (?)", arr(true, false));
            out(db.query("SELECT on_ FROM flags ORDER BY rowid", arr()));
            out(db.value("SELECT count(*) FROM flags WHERE on_ = ?", arr(true)));
        }"#,
        "[{\"on_\": 1}, {\"on_\": 0}]\n1\n");
}

#[test]
fn a_bigint_that_fits_in_64_bits_is_an_integer() {
    executes(r#"
        takepkg std.sqlite;
        takepkg std.types;
        func m{
            db = sqlite.open(":memory:");
            db.run("CREATE TABLE v(x)", arr());
            db.run("INSERT INTO v VALUES (?), (?), (?), (?)",
                arr(bigint(12), bigint("4611686018427387904"), bigint("9223372036854775807"), bigint("-9223372036854775808")));
            for (row, db.query("SELECT x, typeof(x) AS t FROM v ORDER BY rowid", arr())){
                out(types.kind(row.get("x")), row.get("x"), row.get("t"));
            }
        }"#,
        "int 12 integer\nlongint 4611686018427387904 integer\nlongint 9223372036854775807 integer\n\
         longint -9223372036854775808 integer\n");
}

#[test]
fn a_bigint_beyond_64_bits_is_a_range_error_naming_its_position() {
    runtime_error(r#"
        takepkg std.sqlite;
        func m{
            db = sqlite.open(":memory:");
            db.run("SELECT ?, ?", arr(1, bigint("9223372036854775808")));
        }"#,
        "RangeError: SQL parameter 2 is the bigint \"9223372036854775808\", which does not fit in 64 bits");
    runtime_error(r#"
        takepkg std.sqlite;
        func m{
            db = sqlite.open(":memory:");
            db.run("SELECT ?", arr(bigint("-9223372036854775809")));
        }"#,
        "SQL parameter 1 is the bigint \"-9223372036854775809\"");
}

#[test]
fn blobs_come_back_as_arrays_of_byte_ints_and_arrays_of_bytes_are_stored_as_blobs() {
    executes(r#"
        takepkg std.sqlite;
        func m{
            db = sqlite.open(":memory:");
            db.run("CREATE TABLE b(data BLOB)", arr());
            db.run("INSERT INTO b VALUES (?), (?), (?)", arr(arr(0, 1, 127, 128, 255), arr(), arr(65, 66, 67)));
            for (row, db.query("SELECT data, typeof(data) AS t, length(data) AS n FROM b ORDER BY rowid", arr())){
                out(row.get("data"), row.get("t"), row.get("n"));
            }
            out(db.value("SELECT hex(data) FROM b WHERE rowid = 1", arr()));
            out(db.value("SELECT count(*) FROM b WHERE data = ?", arr(arr(65, 66, 67))));
            out(db.value("SELECT x'DEADBEEF'", arr()));
        }"#,
        "[0, 1, 127, 128, 255] blob 5\n[] blob 0\n[65, 66, 67] blob 3\n00017F80FF\n1\n[222, 173, 190, 239]\n");
}

#[test]
fn a_parameter_of_a_kind_sql_cannot_hold_is_a_type_error_naming_its_position() {
    for (parameter, kind) in [
        ("map().set(1, 2)", "map"),
        ("set(1)", "set"),
        ("pair(1, 2)", "pair"),
        ("date(2026, 10, 16)", "date"),
        ("func(int x) => x", "function"),
    ] {
        runtime_error(&format!(r#"
            takepkg std.sqlite;
            func m{{
                db = sqlite.open(":memory:");
                db.run("SELECT ?, ?, ?", arr(1, "two", {parameter}));
            }}"#),
            &format!("TypeError: SQL parameter 3 is a {kind}, which SQLite cannot store"));
    }
}

#[test]
fn an_array_for_a_blob_must_hold_only_byte_ints() {
    runtime_error(r#"
        takepkg std.sqlite;
        func m{
            db = sqlite.open(":memory:");
            db.run("SELECT ?, ?", arr(1, arr(10, 300)));
        }"#,
        "SQL parameter 2 is an array, stored as a blob, which holds ints from 0 to 255, but item 2 is 300");
    runtime_error(r#"
        takepkg std.sqlite;
        func m{
            db = sqlite.open(":memory:");
            db.query("SELECT ?", arr(arr(1, "x")));
        }"#,
        "SQL parameter 1 is an array, stored as a blob, which holds ints from 0 to 255, but item 2 is a string");
    runtime_error(r#"
        takepkg std.sqlite;
        func m{
            db = sqlite.open(":memory:");
            db.value("SELECT ?", arr(arr(-1)));
        }"#,
        "but item 1 is -1");
}

#[test]
fn params_must_be_an_array() {
    runtime_error(r#"
        takepkg std.sqlite;
        func m{
            db = sqlite.open(":memory:");
            db.run("SELECT 1", 5);
        }"#,
        "TypeError");
}

#[test]
fn a_wrong_number_of_parameters_is_a_sql_error_saying_how_many() {
    runtime_error(r#"
        takepkg std.sqlite;
        func m{
            db = sqlite.open(":memory:");
            db.query("SELECT ?, ?", arr(1));
        }"#,
        "SqlError: the SQL has 2 ? placeholders but 1 parameter was given [SQL: SELECT ?, ?]");
    runtime_error(r#"
        takepkg std.sqlite;
        func m{
            db = sqlite.open(":memory:");
            db.run("SELECT 1", arr(1, 2));
        }"#,
        "SqlError: the SQL has 0 ? placeholders but 2 parameters were given");
    runtime_error(r#"
        takepkg std.sqlite;
        func m{
            db = sqlite.open(":memory:");
            db.value("SELECT ?", arr());
        }"#,
        "the SQL has 1 ? placeholder but 0 parameters were given");
}

#[test]
fn a_syntax_error_is_a_sql_error_with_the_sql_shortened_to_80_characters() {
    executes(r#"
        takepkg std.sqlite;
        func m{
            db = sqlite.open(":memory:");
            run{
                db.query("SELEC 1", arr());
            } handle(sqlite.SqlError e){
                out(e.message);
            }
            long = "SELECT 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20 FRM nowhere WHERE x = 1";
            run{
                db.run(long, arr());
            } handle(sqlite.SqlError e){
                out(e.message);
            }
            run{
                db.run("SELECT *\n    FROM nowhere", arr());
            } handle(sqlite.SqlError e){
                out(e.message);
            }
        }"#,
        "near \"SELEC\": syntax error (at offset 0) [SQL: SELEC 1]\n\
         near \"nowhere\": syntax error (at offset 81) [SQL: SELECT 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20 FRM...]\n\
         no such table: nowhere [SQL: SELECT * FROM nowhere]\n");
}

#[test]
fn run_and_query_are_for_their_own_kind_of_statement_and_one_statement_at_a_time() {
    runtime_error(r#"
        takepkg std.sqlite;
        func m{
            db = sqlite.open(":memory:");
            db.run("SELECT 1", arr());
        }"#,
        "the statement returns rows; run is for statements that do not, so use query");
    runtime_error(r#"
        takepkg std.sqlite;
        func m{
            db = sqlite.open(":memory:");
            db.run("CREATE TABLE a(x); CREATE TABLE b(y)", arr());
        }"#,
        "more than one statement; run, query, query_one, and value take one, so use script for several");
    runtime_error(r#"
        takepkg std.sqlite;
        func m{
            db = sqlite.open(":memory:");
            db.value("CREATE TABLE a(x)", arr());
        }"#,
        "the statement returns no columns");
}

#[test]
fn a_unique_violation_is_a_sql_error_with_sqlites_message() {
    executes(r#"
        takepkg std.sqlite;
        func m{
            db = sqlite.open(":memory:");
            db.run("CREATE TABLE users(email TEXT UNIQUE NOT NULL, name TEXT)", arr());
            db.run("INSERT INTO users VALUES (?, ?)", arr("a@b.c", "A"));
            run{
                db.run("INSERT INTO users VALUES (?, ?)", arr("a@b.c", "B"));
            } handle(sqlite.SqlError e){
                out(e.message);
            }
            run{
                db.run("INSERT INTO users VALUES (?, ?)", arr(null, "C"));
            } handle(Error e){
                out(e.message);
            }
            out(db.value("SELECT count(*) FROM users", arr()));
        }"#,
        "UNIQUE constraint failed: users.email\nNOT NULL constraint failed: users.email\n1\n");
    runtime_error(r#"
        takepkg std.sqlite;
        func m{
            db = sqlite.open(":memory:");
            db.script("CREATE TABLE t(id INTEGER PRIMARY KEY); INSERT INTO t VALUES (1);");
            db.run("INSERT INTO t VALUES (1)", arr());
        }"#,
        "SqlError: UNIQUE constraint failed: t.id");
}

#[test]
fn foreign_keys_are_enforced() {
    executes(r#"
        takepkg std.sqlite;
        func m{
            db = sqlite.open(":memory:");
            db.script("CREATE TABLE parent(id INTEGER PRIMARY KEY); CREATE TABLE child(p INTEGER REFERENCES parent(id));");
            db.run("INSERT INTO parent VALUES (1)", arr());
            db.run("INSERT INTO child VALUES (1)", arr());
            run{
                db.run("INSERT INTO child VALUES (?)", arr(2));
            } handle(sqlite.SqlError e){
                out(e.message);
            }
        }"#,
        "FOREIGN KEY constraint failed\n");
}

#[test]
fn query_one_value_and_last_id() {
    executes(r#"
        takepkg std.sqlite;
        func m{
            db = sqlite.open(":memory:");
            db.run("CREATE TABLE t(id INTEGER PRIMARY KEY, name TEXT)", arr());
            out(db.last_id());
            db.run("INSERT INTO t(name) VALUES (?)", arr("a"));
            out(db.last_id());
            db.run("INSERT INTO t(name) VALUES (?)", arr("b"));
            out(db.last_id());
            db.run("INSERT INTO t(id, name) VALUES (?, ?)", arr(5000000000, "big"));
            out(db.last_id());

            out(db.query_one("SELECT * FROM t ORDER BY id", arr()));
            out(db.query_one("SELECT * FROM t WHERE id = ?", arr(2)));
            out(db.query_one("SELECT * FROM t WHERE id = ?", arr(99)));
            out(db.query_one("SELECT name FROM t WHERE id > ? ORDER BY id DESC", arr(0)));

            out(db.value("SELECT count(*) FROM t", arr()));
            out(db.value("SELECT name, id FROM t ORDER BY id DESC", arr()));
            out(db.value("SELECT name FROM t WHERE id = ?", arr(99)));
            out(db.value("SELECT NULL", arr()));
            out(db.value("SELECT 1.5 * 2", arr()));
        }"#,
        "0\n1\n2\n5000000000\n\
         {\"id\": 1, \"name\": \"a\"}\n{\"id\": 2, \"name\": \"b\"}\nnull\n{\"name\": \"big\"}\n\
         3\nbig\nnull\nnull\n3\n");
}

#[test]
fn a_parameter_holding_sql_is_stored_as_plain_text() {
    executes(r#"
        takepkg std.sqlite;
        func m{
            db = sqlite.open(":memory:");
            db.run("CREATE TABLE users(name TEXT)", arr());
            evil = "Robert'); DROP TABLE users; --";
            db.run("INSERT INTO users(name) VALUES (?)", arr(evil));
            out(db.value("SELECT name FROM users", arr()));
            out(db.value("SELECT count(*) FROM users", arr()));
            out(db.query("SELECT name FROM users WHERE name = ?", arr(evil)).len());
            out(db.value("SELECT count(*) FROM sqlite_master WHERE name = 'users'", arr()));
            out(db.query("SELECT name FROM users WHERE name = ?", arr("x' OR '1'='1")).len());
        }"#,
        "Robert'); DROP TABLE users; --\n1\n1\n1\n0\n");
}

#[test]
fn script_runs_several_statements_and_stops_at_the_first_failure() {
    executes(r#"
        takepkg std.sqlite;
        func m{
            db = sqlite.open(":memory:");
            db.script("CREATE TABLE a(x INTEGER); CREATE TABLE b(y TEXT);" + chr(10) + "INSERT INTO a VALUES (1); INSERT INTO a VALUES (2);" + chr(10) + "INSERT INTO b VALUES ('semi;colon');" + chr(10));
            out(db.value("SELECT sum(x) FROM a", arr()));
            out(db.value("SELECT y FROM b", arr()));
            run{
                db.script("INSERT INTO a VALUES (3); INSERT INTO nowhere VALUES (4); INSERT INTO a VALUES (5);");
            } handle(sqlite.SqlError e){
                out(e.message);
            }
            out(db.query("SELECT x FROM a ORDER BY x", arr()));
            out(db.script(""));
        }"#,
        "3\nsemi;colon\nno such table: nowhere [SQL: INSERT INTO a VALUES (3); INSERT INTO nowhere VALUES (4); INSERT INTO a VALUES (...]\n\
         [{\"x\": 1}, {\"x\": 2}, {\"x\": 3}]\nnull\n");
}

#[test]
fn a_transaction_commits_when_the_function_returns() {
    executes(r#"
        takepkg std.sqlite;
        func m{
            db = sqlite.open(":memory:");
            db.run("CREATE TABLE t(n INTEGER)", arr());
            func add(){
                db.run("INSERT INTO t VALUES (1)", arr());
                db.run("INSERT INTO t VALUES (2)", arr());
                ret "done";
            }
            out(db.transaction(add));
            out(db.value("SELECT sum(n) FROM t", arr()));
            out(db.transaction(func() => db.value("SELECT count(*) FROM t", arr())));
        }"#,
        "done\n3\n2\n");
}

#[test]
fn a_transaction_rolls_back_and_raises_the_same_error_when_the_function_raises() {
    executes(r#"
        takepkg std.sqlite;
        family OutOfMoney(Error){}
        func m{
            db = sqlite.open(":memory:");
            db.script("CREATE TABLE t(n INTEGER); INSERT INTO t VALUES (10);");
            func spend(){
                db.run("UPDATE t SET n = n - 30", arr());
                if (db.value("SELECT n FROM t", arr()) < 0){ err(OutOfMoney, "short by " + string(-db.value("SELECT n FROM t", arr()))); }
            }
            run{
                db.transaction(spend);
            } handle(OutOfMoney e){
                out("caught", e.message);
            }
            out(db.value("SELECT n FROM t", arr()));

            func bad(){
                db.run("INSERT INTO t VALUES (1)", arr());
                db.run("INSERT INTO nowhere VALUES (1)", arr());
            }
            run{
                db.transaction(bad);
            } handle(sqlite.SqlError e){
                out(e.message);
            }
            out(db.value("SELECT count(*) FROM t", arr()));

            func divide(){
                db.run("INSERT INTO t VALUES (1)", arr());
                out(1 / 0);
            }
            run{
                db.transaction(divide);
            } handle(RangeError e){
                out("range error");
            }
            out(db.value("SELECT count(*) FROM t", arr()));

            db.transaction(func() => db.run("INSERT INTO t VALUES (7)", arr()));
            out(db.value("SELECT count(*) FROM t", arr()));
        }"#,
        "caught short by 20\n10\n\
         no such table: nowhere [SQL: INSERT INTO nowhere VALUES (1)]\n1\n\
         range error\n1\n2\n");
}

#[test]
fn an_unhandled_error_in_a_transaction_goes_on_to_the_caller_unchanged() {
    runtime_error(r#"
        takepkg std.sqlite;
        func m{
            db = sqlite.open(":memory:");
            db.run("CREATE TABLE t(email TEXT UNIQUE)", arr());
            db.run("INSERT INTO t VALUES ('a')", arr());
            func again(){ db.run("INSERT INTO t VALUES ('a')", arr()); }
            db.transaction(again);
        }"#,
        "SqlError: UNIQUE constraint failed: t.email");
}

#[test]
fn a_transaction_that_already_ended_itself_does_not_hide_the_error() {
    executes(r#"
        takepkg std.sqlite;
        func m{
            db = sqlite.open(":memory:");
            db.run("CREATE TABLE t(n)", arr());
            func rude(){
                db.run("INSERT INTO t VALUES (1)", arr());
                db.run("ROLLBACK", arr());
                err(Error, "original");
            }
            run{
                db.transaction(rude);
            } handle(Error e){
                out(e.message);
            }
            out(db.value("SELECT count(*) FROM t", arr()));
            func fresh(){ db.run("INSERT INTO t VALUES (2)", arr()); }
            db.transaction(fresh);
            out(db.value("SELECT count(*) FROM t", arr()));
        }"#,
        "original\n0\n1\n");
}

#[test]
fn a_nested_transaction_is_a_savepoint_that_can_fail_alone() {
    executes(r#"
        takepkg std.sqlite;
        func m{
            db = sqlite.open(":memory:");
            db.run("CREATE TABLE t(n INTEGER)", arr());
            func inner_ok(){ db.run("INSERT INTO t VALUES (2)", arr()); }
            func inner_bad(){
                db.run("INSERT INTO t VALUES (3)", arr());
                err(Error, "inner failed");
            }
            func outer(){
                db.run("INSERT INTO t VALUES (1)", arr());
                db.transaction(inner_ok);
                run{
                    db.transaction(inner_bad);
                } handle(Error e){
                    out("handled", e.message);
                }
                db.run("INSERT INTO t VALUES (4)", arr());
            }
            db.transaction(outer);
            out(db.query("SELECT n FROM t ORDER BY n", arr()));
        }"#,
        "handled inner failed\n[{\"n\": 1}, {\"n\": 2}, {\"n\": 4}]\n");
}

#[test]
fn an_unhandled_error_in_a_nested_transaction_rolls_back_the_outer_one_too() {
    executes(r#"
        takepkg std.sqlite;
        func m{
            db = sqlite.open(":memory:");
            db.run("CREATE TABLE t(n INTEGER)", arr());
            func inner(){
                db.run("INSERT INTO t VALUES (2)", arr());
                err(RangeError, "deep");
            }
            func outer(){
                db.run("INSERT INTO t VALUES (1)", arr());
                db.transaction(inner);
            }
            run{
                db.transaction(outer);
            } handle(RangeError e){
                out(e.message);
            }
            out(db.value("SELECT count(*) FROM t", arr()));
            func three(){
                func two(){
                    func one(){ db.run("INSERT INTO t VALUES (9)", arr()); }
                    db.transaction(one);
                }
                db.transaction(two);
            }
            db.transaction(three);
            out(db.value("SELECT count(*) FROM t", arr()));
        }"#,
        "deep\n0\n1\n");
}

#[test]
fn using_a_closed_database_raises_sql_error() {
    for call in [
        r#"db.run("SELECT 1", arr())"#,
        r#"db.query("SELECT 1", arr())"#,
        r#"db.query_one("SELECT 1", arr())"#,
        r#"db.value("SELECT 1", arr())"#,
        "db.last_id()",
        r#"db.script("SELECT 1")"#,
        "db.transaction(func() => 1)",
    ] {
        runtime_error(&format!(r#"
            takepkg std.sqlite;
            func m{{
                db = sqlite.open(":memory:");
                db.close();
                {call};
            }}"#),
            "SqlError: the database :memory: is closed; open it again with sqlite.open");
    }
}

#[test]
fn closing_twice_is_harmless_and_a_closed_in_memory_database_is_gone() {
    executes(r#"
        takepkg std.sqlite;
        func m{
            db = sqlite.open(":memory:");
            db.run("CREATE TABLE t(x)", arr());
            db.close();
            db.close();
            fresh = sqlite.open(":memory:");
            out(fresh.value("SELECT count(*) FROM sqlite_master", arr()));
            run{
                db.value("SELECT 1", arr());
            } handle(sqlite.SqlError e){
                out("still closed");
            }
            out(fresh.value("SELECT 1", arr()));
        }"#,
        "0\nstill closed\n1\n");
}

#[test]
fn closing_one_database_leaves_another_open() {
    executes(r#"
        takepkg std.sqlite;
        func m{
            a = sqlite.open(":memory:");
            b = sqlite.open(":memory:");
            a.close();
            out(b.value("SELECT 2", arr()));
            run{ a.value("SELECT 1", arr()); } handle(sqlite.SqlError e){ out("a closed"); }
        }"#,
        "2\na closed\n");
}

#[test]
fn two_databases_open_at_once_are_separate() {
    let dir = TempDir::new("two");
    let path = dir.literal("one.db");
    executes(&format!(r#"
        takepkg std.sqlite;
        func m{{
            a = sqlite.open(":memory:");
            b = sqlite.open(":memory:");
            c = sqlite.open({path});
            for (db, arr(a, b, c)){{ db.run("CREATE TABLE t(who TEXT)", arr()); }}
            a.run("INSERT INTO t VALUES (?)", arr("a"));
            b.run("INSERT INTO t VALUES (?)", arr("b1"));
            b.run("INSERT INTO t VALUES (?)", arr("b2"));
            c.run("INSERT INTO t VALUES (?)", arr("c"));
            out(a.query("SELECT who FROM t", arr()));
            out(b.value("SELECT count(*) FROM t", arr()));
            out(c.query("SELECT who FROM t", arr()));
            out(a.last_id(), b.last_id(), c.last_id());

            func in_a(){{
                a.run("INSERT INTO t VALUES ('a2')", arr());
                b.run("INSERT INTO t VALUES ('b3')", arr());
                err(Error, "stop");
            }}
            run{{ a.transaction(in_a); }} handle(Error e){{ out(e.message); }}
            out(a.value("SELECT count(*) FROM t", arr()), b.value("SELECT count(*) FROM t", arr()));
        }}"#),
        "[{\"who\": \"a\"}]\n2\n[{\"who\": \"c\"}]\n1 2 1\nstop\n1 3\n");
}

#[test]
fn the_same_file_opened_twice_shares_its_data() {
    let dir = TempDir::new("shared");
    let path = dir.literal("shared.db");
    executes(&format!(r#"
        takepkg std.sqlite;
        func m{{
            first = sqlite.open({path});
            second = sqlite.open({path});
            first.run("CREATE TABLE t(x)", arr());
            first.run("INSERT INTO t VALUES (1)", arr());
            out(second.value("SELECT count(*) FROM t", arr()));
        }}"#),
        "1\n");
}

#[test]
fn opening_a_path_in_a_missing_directory_raises_the_std_file_error() {
    let dir = TempDir::new("missing");
    let path = dir.literal("no/such/dir/x.db");
    runtime_error(&format!(r#"
        takepkg std.sqlite;
        func m{{ sqlite.open({path}); }}"#),
        "NotFoundError");
    runtime_error(&format!(r#"
        takepkg std.sqlite;
        func m{{ sqlite.open({path}); }}"#),
        "the directory");
    executes(&format!(r#"
        takepkg std.sqlite;
        takepkg std.file;
        func m{{
            run{{
                sqlite.open({path});
            }} handle(file.FileError e){{
                out("a file error");
            }}
        }}"#),
        "a file error\n");
}

#[test]
fn opening_a_directory_or_an_empty_path_raises() {
    let dir = TempDir::new("isdir");
    std::fs::create_dir(dir.path("sub")).unwrap();
    let path = dir.literal("sub");
    runtime_error(&format!(r#"
        takepkg std.sqlite;
        func m{{ sqlite.open({path}); }}"#),
        "IsDirectoryError");
    runtime_error(r#"
        takepkg std.sqlite;
        func m{ sqlite.open(""); }"#,
        "RangeError: sqlite.open needs a file path or \":memory:\", got an empty string");
}

#[test]
fn opening_a_file_that_is_not_a_database_raises_sql_error_when_it_is_used() {
    let dir = TempDir::new("junk");
    std::fs::write(dir.path("junk.db"), b"this is not a database file, just some text that is long enough to look like a header").unwrap();
    let path = dir.literal("junk.db");
    executes(&format!(r#"
        takepkg std.sqlite;
        func m{{
            db = sqlite.open({path});
            run{{
                db.value("SELECT count(*) FROM sqlite_master", arr());
            }} handle(sqlite.SqlError e){{
                out(e.message);
            }}
        }}"#),
        "file is not a database [SQL: SELECT count(*) FROM sqlite_master]\n");
}

#[test]
fn text_with_a_nul_character_and_long_text_round_trip() {
    executes(r#"
        takepkg std.sqlite;
        takepkg std.strings;
        func m{
            db = sqlite.open(":memory:");
            big = strings.repeated("ab", 100000);
            db.run("CREATE TABLE t(x TEXT)", arr());
            db.run("INSERT INTO t VALUES (?), (?)", arr(big, chr(0) + "z"));
            out(db.value("SELECT length(x) FROM t ORDER BY rowid", arr()));
            out(db.query_one("SELECT x FROM t WHERE rowid = 1", arr()).get("x") == big);
            nul = db.value("SELECT x FROM t WHERE rowid = 2", arr());
            out(nul.len(), ord(nul.slice(0, 1)), nul.slice(1, 2));
        }"#,
        "200000\ntrue\n2 0 z\n");
}

#[test]
fn a_database_left_open_is_closed_when_the_program_ends() {
    // In WAL mode SQLite keeps a -wal file while any connection is open and removes it when the
    // last one closes. A program that never calls close must leave neither it nor a lock behind.
    let dir = TempDir::new("leftopen");
    let path = dir.literal("left.db");
    executes(&format!(r#"
        takepkg std.sqlite;
        takepkg std.file;
        func m{{
            db = sqlite.open({path});
            db.value("PRAGMA journal_mode = wal", arr());
            db.script("CREATE TABLE t(x); INSERT INTO t VALUES (1);");
            out(file.exists({path} + "-wal"));
        }}"#),
        "true\n");
    assert!(dir.path("left.db").is_file());
    assert!(!dir.path("left.db-wal").exists(), "the connection was still open after the program ended");
    assert!(!dir.path("left.db-shm").exists(), "the connection was still open after the program ended");
    // Nothing holds the file: on Windows an open handle would make this fail.
    std::fs::remove_file(dir.path("left.db")).expect("the file is free to delete");
}

#[test]
fn a_database_left_open_is_closed_when_the_program_ends_with_an_error() {
    let dir = TempDir::new("lefterror");
    let path = dir.literal("left.db");
    runtime_error(&format!(r#"
        takepkg std.sqlite;
        func m{{
            db = sqlite.open({path});
            db.value("PRAGMA journal_mode = wal", arr());
            db.script("CREATE TABLE t(x)");
            err(Error, "the program fails");
        }}"#),
        "the program fails");
    assert!(!dir.path("left.db-wal").exists(), "the connection was still open after the program failed");
    std::fs::remove_file(dir.path("left.db")).expect("the file is free to delete");
}

#[test]
fn a_transaction_open_when_the_program_ends_is_rolled_back() {
    let dir = TempDir::new("uncommitted");
    let path = dir.literal("u.db");
    executes(&format!(r#"
        takepkg std.sqlite;
        func m{{
            db = sqlite.open({path});
            db.run("CREATE TABLE t(x)", arr());
            db.run("BEGIN", arr());
            db.run("INSERT INTO t VALUES (1)", arr());
        }}"#),
        "");
    executes(&format!(r#"
        takepkg std.sqlite;
        func m{{
            db = sqlite.open({path});
            out(db.value("SELECT count(*) FROM t", arr()));
        }}"#),
        "0\n");
}

#[test]
fn closing_in_the_middle_of_a_transaction_discards_it() {
    executes(r#"
        takepkg std.sqlite;
        func m{
            db = sqlite.open(":memory:");
            db.run("CREATE TABLE t(x)", arr());
            func leave(){
                db.run("INSERT INTO t VALUES (1)", arr());
                db.close();
            }
            run{ db.transaction(leave); } handle(sqlite.SqlError e){ out(e.message); }
        }"#,
        "the database :memory: is closed; open it again with sqlite.open\n");
}

#[test]
fn the_database_family_has_its_path() {
    executes(r#"
        takepkg std.sqlite;
        func m{
            db = sqlite.open(":memory:");
            out(db.path);
        }"#,
        ":memory:\n");
}
