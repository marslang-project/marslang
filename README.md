# marslang

Marslang is a Rust implementation of the **marslang rs-0.5.0** compiler.

See the [rs-0.5.0 release notes](docs/releases/rs-0.5.0.md) for the initial math package, dynamic numeric tracking, and validation.

The development direction is to stabilize the Rust-to-JavaScript compiler before self-hosting. Discussion documents under `docs/` stay local; `docs/api/` and `docs/releases/` are available to Git.

Read the [language and API reference](docs/api/README.md) for current syntax and built-in APIs.

Run `cargo test` for current coverage, including Node execution tests. All existing regression cases are enabled. Node must be on PATH, or set `MARSLANG_NODE` to its executable path.

This release adds dynamically tracked numeric types and a Marslang-written
[`std.math`](docs/api/std-math.md). The math API is an initial design and will expand.
Decorators, deque, further standard
libraries, complete module handling, and async remain pending. Maps, deep copies,
and loop control are implemented. `:=` is excluded.

Function parameters now use commas: `func add(int a, int b) => a + b;`. The parser accepts this form and rejects semicolons inside parameter lists, including after the last parameter. The decorator package is `std.Decorator`; decorator execution remains pending.

It includes:
- a lexer (`src/lexer.rs`)
- AST definitions (`src/ast.rs`)
- a handwritten parser (`src/parser.rs`); `src/grammar.pest` is an unused reference grammar
- a compiler/transpiler to JavaScript (`src/eval.rs`)
- CLI executable named `marslang` (`src/main.rs`)

## Build

```bash
cargo build
```

## Compile a `.mars` file

```bash
cargo run -- hello.mars
# or
cargo run -- compile hello.mars -o hello.js
```

Then run the generated JavaScript:

```bash
node hello.js
```

## Current release coverage

Implemented:
- lexical binding resolution: bare assignment updates the nearest binding or declares a local; annotations and `cold` declare explicitly
- `fixed` bindings and recursively immutable containers; direct fixed-copy propagation and `.copy()` for independent mutable deep copies
- constant-expression substitution for hot variables; runtime type checks, signed 32/64-bit integer bounds, and identity-based collection equality
- `func` block and expression forms
- `family` definitions with optional inheritance and `init` -> constructor mapping
- `ret`, assignment, function calls, member access
- `if/elif/else`, `repeat`, `while`, two- and three-part `for`, `break`, and `continue`
- `takepkg module;` and `takepkg module = alias;`
- built-in `arr`, `set`, `pair`, `map`/`dict`; legacy `a`/`s`/`p` remain compatibility spellings
- array operations, collection `.len()`/`.is_empty()`/`.has()`, insertion-ordered map keys and snapshot iteration
- `out`, `slout`, `in`, and `inln`
- `fasle` accepted as `false`

Still intentionally limited in this initial release:
- no full static type checker yet
- no dedicated bytecode/native backend yet (current backend is JS)
- no complete `match`, `run/handle/then`, hot functions, type aliases, or tagged-variant syntax yet
- the statement parser still uses normalized source fragments; the standalone lexer is not yet the single compilation frontend

```mars
func m{
    counts = map();
    counts.set("apples", 3);
    for (key, counts){
        out(counts.get(key));
    }
}
```

Maps use `.set(key,value)`, `.get(key)` (null when absent), `.has(key)`, `.len()`,
`.keys()`, `.values()`, and `.remove(key)`. Iteration visits keys in insertion order.
Loop iterables are snapshotted; mutations do not change the current iteration list.

String `.len()` and iteration count Unicode grapheme clusters: `é`, `中`, and
`👨‍👩‍👧‍👦` each count as one character. This requires Node with `Intl.Segmenter`.
String `.reverse()` returns a new string with those characters in reverse order;
the original string is unchanged.
String and array `.slice(start,end)` and `.lenslice(start,length)` support an
optional third argument `reverse=true`, which counts from the end while preserving
the selected characters' or elements' order. Invalid bounds raise `OutOfBoundsError`.
See the [string API](docs/api/strings.md) for Unicode and boundary examples.

## Example

```mars
fixed hot pi (float) = 3.14159;

func area(float r) => pi * r * r;

family Circle{
    func init(float r){
        me.r = r;
    }

    func size(){
        ret area(me.r);
    }
}

func m{
    nums (array[int]) = arr(3,1,4,1,5);
    out(nums.iget(0));

    c = Circle(5);
    out(c.size());
}
```

A starter marslang stdlib is included at `stdlib.mars`.


## REPL

```bash
cargo run -- repl
```

The REPL keeps a stateful source buffer and re-runs it after each line. Use `:show`, `:reset`, and `:exit`.

## Building a Windows `.exe`

From Windows (or with a Windows target toolchain installed), build:

```bash
cargo build --release
```

The executable will be at `target/release/marslang.exe` on Windows.
