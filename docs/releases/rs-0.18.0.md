# rs-0.18.0 — match, SQLite, and secure tokens

`match` replaces long `if`/`elif` chains, [std.sqlite](../api/std-sqlite.md)
puts a real SQL database in one file, and [std.secrets](../api/std-secrets.md)
and [std.uuid](../api/std-uuid.md) make tokens, passwords, and IDs that are
safe to trust.

## match

```mars
family Circle{
    func init(float r){ me.r = r; }
}

func describe(any value) => match (value){
    1, 2, 3 => "small";
    int n if (n > 100) => "big";
    string s => "text " + s;
    Circle c => "a circle";
    _ => "something else";
};

func m{
    out(describe(2), describe(500), describe("hi"), describe(Circle(1.0)));   // small big text hi a circle
}
```

Arms are tried top to bottom and the first that matches runs. Patterns are
literals and lists of them, compared with `==`; `_`; type patterns that bind a
name, such as `int n`, `string s`, `Circle c`, or `[int, float] n`; and guards
with `if (...)`. As a statement, a value no arm matches does nothing; as an
expression, it raises `RangeError` naming the value. Unreachable arms, repeated
literals, and unknown types are compile errors with their line. Matching is
faster than the `if`/`elif` chain it replaces. See
[match](../api/language.md#match).

**Compatibility:** `match` is now a keyword, so a variable, function, or
parameter named `match` must be renamed. `obj.match` as a member still works.

## std.sqlite

```mars
takepkg std.sqlite;

func m{
    db = sqlite.open(":memory:");
    db.run("CREATE TABLE people (name TEXT, age INTEGER)", arr());
    db.run("INSERT INTO people VALUES (?, ?)", arr("Ada", 36));
    out(db.query("SELECT name, age FROM people", arr()));   // [{"name": "Ada", "age": 36}]
    db.close();
}
```

`open` a file or `":memory:"`; `run`, `query`, `query_one`, `value`,
`last_id`, `script`, and `transaction`, which rolls back when its function
raises and uses savepoints when nested. Values go in only through `?`
parameters, so text can never be run as SQL. SQLite itself is built in.

## std.secrets and std.uuid

`secrets.token_hex(16)`, `token_url`, `password(20)`, `int_below`, `choice`,
and `same` use the operating system's secure random source: use them, never
std.random, for passwords, keys, and tokens. `uuid.v4()` makes random IDs and
`uuid.v7()` time-ordered ones, with `valid` and `version` to check them.

## Validation

`cargo test` passes 395 tests on Linux and 393 on Windows. Every push runs them
on Windows, macOS, and Linux on x86_64 and ARM64, and runs the static Linux
binary inside Alpine, Debian 11, Ubuntu 20.04, Rocky Linux 8, and Amazon
Linux 2.
