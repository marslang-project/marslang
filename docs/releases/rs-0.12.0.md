# rs-0.12.0 — Functions as values, string methods, and std.cli

Functions can now be written inline and passed around, strings gain the methods
text processing needs, and programs can read their command line. The syntax is
written down in full in the new [grammar](../api/grammar.md) page.

## Anonymous functions and closures

`func(type name) => expression` makes a function without a name, and a `func`
declared inside a block is a local function. Both close over the variables
around them by reference, so a counter keeps its count:

```mars
func apply(Function f, int x) => f(x);

func counter(){
    count = 0;
    func next(){
        count = count + 1;
        ret count;
    }
    ret next;
}

func m{
    double = func(int x) => x * 2;
    out(apply(double, 21));   // 42
    tick = counter();
    tick(); tick();
    out(tick());              // 3
}
```

`Function` and `Family` are parameter types for code that takes a function or
a family as an argument; they are not allowed on variables. Inside a method, a
closure keeps `me` and the family's private access. Closures made in a loop
share the loop variable, as in Python; see
[functions as values](../api/language.md#functions-as-values).

## Strings

`s[i]` and `a[i]` read one character or element, counting whole characters,
and strings gain `iget(i)` to match arrays. Indexing is read-only: change an
array element with `modify(i, value)`.

| Method | Result |
| --- | --- |
| `split()`, `split(sep)` | Pieces around whitespace, or around `sep` |
| `lines()` | The lines, without their line endings |
| `strip()`, `lstrip()`, `rstrip()` | Without surrounding whitespace, or the given characters |
| `upper()`, `lower()` | Changed case |
| `replace(old, new)` | Every `old` replaced by `new` |
| `find(x)`, `rfind(x)` | The first or last position of `x`, or `-1` |
| `contains(x)`, `starts_with(x)`, `ends_with(x)` | Booleans |

```mars
out("  name, age ,city  ".strip().split(","));   // ["name", " age ", "city"]
out("héllo👋"[5]);                               // 👋
```

## std.cli

Words after the program's file are now the program's arguments:
`marslang greet.mars --name Ada --loud`. `std.cli` reads them with `args()`,
parses `--flags`, `--options`, and positionals with `cli.parser`, prints a
generated `--help`, and stops with an exit status through `cli.exit(code)`.
An exit passes every `handle`, but `then` blocks still run. See
[std.cli](../api/std-cli.md).

## Integer literals beyond 32 bits

A whole-number literal that does not fit in 32 bits is now a `longint`, so
`3000000000 + 1` is `3000000001`. It used to be an `int` that printed but
raised `int overflow` in any arithmetic. Passing one to an `int` parameter
still raises `int overflow`.

## Fixes

- `marslang lex` understands triple-quoted strings and escaped quotes.
- The README no longer describes the package directory as planned.
- Every push is now tested on Linux and Windows.

## Validation

`cargo test` passes 126 tests (13 unit, 112 execution, 1 memory) on Windows
Rust and on WSL-native Rust.
The memory test now covers cycles through closures.
