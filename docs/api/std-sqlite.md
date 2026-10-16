# std.sqlite

A real SQL database in one file, or in memory, through
[SQLite](https://www.sqlite.org). It needs no server and no installation: the
database is the file, and SQLite is part of the interpreter.

```mars
takepkg std.sqlite;

func m{
    db = sqlite.open(":memory:");
    db.script("CREATE TABLE users(id INTEGER PRIMARY KEY, name TEXT, age INTEGER)");

    db.run("INSERT INTO users(name, age) VALUES (?, ?)", arr("Ada", 36));
    db.run("INSERT INTO users(name, age) VALUES (?, ?)", arr("Alan", 41));
    out(db.last_id());                                                  // 2

    for (row, db.query("SELECT name, age FROM users WHERE age > ?", arr(30))){
        out(row.get("name"), row.get("age"));                           // Ada 36, then Alan 41
    }
    out(db.value("SELECT count(*) FROM users", arr()));                 // 2
    out(db.query_one("SELECT * FROM users WHERE id = ?", arr(9)));      // null
    db.close();
}
```

| Function or method | Result |
| --- | --- |
| `open(path)` | A `Database`; `":memory:"` is a database in memory, and a missing file is created |
| `db.run(sql, params)` | Runs a statement that returns no rows; the number of rows it changed |
| `db.query(sql, params)` | The rows, as an array of maps from column name to value |
| `db.query_one(sql, params)` | The first row as a map, or `null` |
| `db.value(sql, params)` | The first column of the first row, or `null` |
| `db.last_id()` | The rowid of the last row inserted |
| `db.script(sql)` | Runs several statements separated by `;`; no parameters |
| `db.transaction(f)` | Calls `f` between BEGIN and COMMIT, or ROLLBACK if it raises |
| `db.close()` | Closes the database |
| `SqlError` | What SQLite refused |

## Parameters, and never building SQL from text

Put a `?` in the SQL where a value goes, and the values in an array, in order.
`arr()` means none. This is the only way to get values into a statement:

```mars
takepkg std.sqlite;

func m{
    db = sqlite.open(":memory:");
    db.run("CREATE TABLE users(name TEXT)", arr());

    // Parameters: the text below is stored exactly as it is.
    name = "Robert'); DROP TABLE users; --";
    db.run("INSERT INTO users(name) VALUES (?)", arr(name));
    out(db.value("SELECT name FROM users", arr()));    // Robert'); DROP TABLE users; --
    out(db.value("SELECT count(*) FROM users", arr()));  // 1
}
```

**Do not build SQL with `+` or an f-string** (`"... WHERE name = '" + name + "'"`).
Whoever controls `name` can then end the string and write SQL of their own: this
is SQL injection, and it deletes tables and leaks data. A parameter is sent to
SQLite apart from the statement, so it can only ever be a value, never SQL.
Placeholders stand for values only; a table or column name cannot be a
parameter. When one must vary, choose it from a fixed list in the program
rather than pass input into the SQL.

The number of parameters must equal the number of `?`; otherwise `SqlError`
says how many of each there were.

## Values

| Marslang | Stored in SQLite as | Read back as |
| --- | --- | --- |
| `int`, `longint` | INTEGER | `int` when it fits in 32 bits, else `longint` |
| `bigint` that fits in 64 bits | INTEGER | the same |
| `bigint` that does not | `RangeError` | |
| `float` | REAL | `float` |
| `string` | TEXT | `string` |
| `null` | NULL | `null` |
| `bool` | INTEGER, `1` or `0` | `int`: SQLite has no bool, so `true` comes back as `1` |
| array of ints from 0 to 255 | BLOB | array of byte ints |
| anything else | `TypeError` | |

```mars
takepkg std.sqlite;
takepkg std.types;

func m{
    db = sqlite.open(":memory:");
    db.run("CREATE TABLE v(x)", arr());
    db.run("INSERT INTO v VALUES (?), (?), (?)", arr(5000000000, 2.5, arr(222, 173, 190, 239)));
    for (row, db.query("SELECT x, typeof(x) AS type FROM v", arr())){
        out(types.kind(row.get("x")), row.get("x"), row.get("type"));
    }
    // longint 5000000000 integer
    // float 2.5 real
    // array [222, 173, 190, 239] blob
}
```

Integers are exact to 64 bits, like `longint`. A `bigint` beyond that raises
`RangeError`; store it with `string(n)` and read it back with `bigint(text)`.
A parameter of another kind, such as a map, a date, or a function, raises
`TypeError` naming its position, counting from 1: `SQL parameter 3 is a map,
which SQLite cannot store`. Convert a date with `string(d)` first. An array
that holds anything but ints from 0 to 255 raises `TypeError` saying which item.
SQLite stores a float `NaN` as `NULL`.

Text is UTF-8 and may contain any character, including `chr(0)`. A column
whose text is not valid UTF-8 raises `SqlError` when read.

A row is a map in column order. If two columns share a name, as in
`SELECT a.id, b.id`, the map keeps the first position and the last value, so
rename one with `AS`.

## Transactions

```mars
takepkg std.sqlite;

func m{
    db = sqlite.open(":memory:");
    db.script("CREATE TABLE accounts(id INTEGER PRIMARY KEY, balance INTEGER); INSERT INTO accounts VALUES (1, 100), (2, 0);");

    func transfer(){
        db.run("UPDATE accounts SET balance = balance - ? WHERE id = ?", arr(30, 1));
        db.run("UPDATE accounts SET balance = balance + ? WHERE id = ?", arr(30, 2));
    }
    db.transaction(transfer);
    out(db.query("SELECT balance FROM accounts ORDER BY id", arr()));   // [{"balance": 70}, {"balance": 30}]

    func too_much(){
        db.run("UPDATE accounts SET balance = balance - 500 WHERE id = 1", arr());
        err(RangeError, "not enough money");
    }
    run{
        db.transaction(too_much);
    } handle(RangeError e){
        out(e.message);                                                  // not enough money
    }
    out(db.value("SELECT balance FROM accounts WHERE id = 1", arr()));   // 70
}
```

`transaction(f)` runs BEGIN, calls `f` with no arguments, and runs COMMIT; it
returns what `f` returns. If `f` raises, it runs ROLLBACK, so none of `f`'s
changes stay, and raises the same error again. Without a transaction SQLite
commits each statement on its own, so one that must be all-or-nothing, or that
inserts many rows (which is much faster in one transaction), belongs in one.

**Nested transactions** are allowed and use SQLite's SAVEPOINT. A transaction
inside `f` can fail alone: if it raises and `f` handles the error, only the
inner changes are undone and the outer transaction goes on. If nothing handles
the error, it reaches the outer `transaction`, which rolls everything back.
Nothing is committed until the outermost one finishes.

If a COMMIT fails (for example a deferred foreign key), the transaction is rolled
back and the error raised. Do not run `COMMIT` or `ROLLBACK` yourself inside `f`;
if you do, `transaction` notices that nothing is left to undo and still raises
`f`'s own error, but a normal return then fails at its COMMIT.

## Errors

`SqlError` is what SQLite refused, with its own message:

```mars
takepkg std.sqlite;

func m{
    db = sqlite.open(":memory:");
    db.run("CREATE TABLE users(email TEXT UNIQUE)", arr());
    db.run("INSERT INTO users VALUES (?)", arr("a@b.c"));
    run{
        db.run("INSERT INTO users VALUES (?)", arr("a@b.c"));
    } handle(sqlite.SqlError e){
        out(e.message);   // UNIQUE constraint failed: users.email
    }
    run{
        db.query("SELEC * FROM users", arr());
    } handle(sqlite.SqlError e){
        out(e.message);   // near "SELEC": syntax error (at offset 0) [SQL: SELEC * FROM users]
    }
}
```

Besides syntax errors and constraint violations (UNIQUE, NOT NULL, CHECK,
foreign keys, and so on), `SqlError` is raised for a missing table or column,
a parameter count that does not match, a statement of the wrong kind for the
method, a locked database, a file that is not a database, and a closed
database. Every message except a constraint's ends with the SQL, on one line
and cut to 80 characters, as `[SQL: ...]`. Catching `SqlError` catches all of
these; to tell a constraint failure apart, look at the message.

- `run` is for statements that return no rows, and raises `SqlError` for one that
  does, such as SELECT: use `query`. The other methods take any statement.
- `run`, `query`, `query_one`, and `value` take one statement. Several at once
  raise `SqlError` pointing to `script`.
- `script` stops at the first statement that fails; the ones before it have
  already run unless a transaction is open.

**Opening** a path in a directory that does not exist raises
`file.NotFoundError`, and a path that is a directory `file.IsDirectoryError`: these
are facts about the file system, so they are the `std.file` errors, and
`handle(file.FileError)` catches them with the other file problems. An empty
path raises `RangeError`. Any other refusal by SQLite, such as no permission to
write, raises `SqlError`. A file that exists but is not a database opens, and
raises `SqlError` ("file is not a database") at the first statement.

A parameter of a type SQLite cannot hold raises `TypeError`, and a `bigint` too
large raises `RangeError`, whether or not SQL is involved.

## Files, closing, and several databases

```mars
takepkg std.sqlite;
takepkg std.file;

func m{
    path = file.temp_dir() + "/std-sqlite-example.db";
    db = sqlite.open(path);
    db.script("CREATE TABLE IF NOT EXISTS notes(body TEXT)");
    db.run("INSERT INTO notes VALUES (?)", arr("remember"));
    db.close();

    again = sqlite.open(path);                                  // the data is still there
    out(again.value("SELECT count(*) FROM notes", arr()));      // 1
    again.close();
    file.remove(path);
}
```

`open` creates the file if it is missing; the directory must exist. Opening the
same file twice gives two connections to it, which see each other's committed
changes; SQLite waits up to five seconds for a lock another connection holds
before raising `SqlError`. Each `":memory:"` is a separate database that is gone
when closed.

`close()` closes the database, discarding anything uncommitted. Closing twice
does nothing; any other use of a closed database raises `SqlError`. A database
never closed is closed when the program ends, even if it ends with an error, and
a transaction left open then is rolled back. Close one you are finished with
anyway, to free the file.

Foreign keys are enforced: `open` turns on `PRAGMA foreign_keys`, which SQLite
itself leaves off. Run `PRAGMA foreign_keys = OFF` to change that. A PRAGMA that
returns a value, such as `PRAGMA journal_mode`, is read with `value` or `query`,
not `run`.

## How it works

SQLite is compiled into the interpreter from its C source
([rusqlite](https://docs.rs/rusqlite) with the `bundled` feature), so nothing
needs to be installed and every platform gets the same SQLite version. Opening
and closing, running a statement with its parameters, reading rows, and
converting values are native ([std/rs/sqlite.rs](../../std/rs/sqlite.rs)). A
native package cannot hold a Marslang object, so it keeps each open connection
in a table under a number, and a `Database` holds that number; closing removes
the entry, and a number not in the table is a closed database. The table belongs
to the program's thread, so it is dropped, closing every connection, when the
program ends. `Database`, `transaction` with its nesting, and raising the errors
are Marslang, in [std/sqlite.mars](../../std/sqlite.mars).
