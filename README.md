# marslang

Marslang is an interpreted language. This repository is its Rust implementation (**marslang rs-0.8.1**): it parses and checks a `.mars` program, then runs it directly. No JavaScript or Node is involved.

See the [rs-0.8.1 release notes](docs/releases/rs-0.8.1.md) for access and type-check fixes, the [rs-0.8.0 release notes](docs/releases/rs-0.8.0.md) for private methods, the [rs-0.7.0 release notes](docs/releases/rs-0.7.0.md) for error handling and new standard packages, and the [rs-0.6.0 release notes](docs/releases/rs-0.6.0.md) for the Rust interpreter and package system.

The development direction is to stabilize the Rust interpreter before self-hosting. Discussion documents under `docs/` stay local; `docs/api/` and `docs/releases/` are available to Git.

Read the [language and API reference](docs/api/README.md) for current syntax and built-in APIs.
The same reference is published at [marslang.kevin-z.com](https://marslang.kevin-z.com); the site
generator lives in [marslang-project/website](https://github.com/marslang-project/website).

Run `cargo test` for current coverage. The execution tests run each program in the interpreter and assert its output or runtime error. Only Rust is required.

This release adds private methods through `std.Decorator` (`@Decorator.private`,
`@Decorator.subclass`) and fixes two crash paths and four correctness issues from the
rs-0.7.0 review. Earlier releases added error handling, cycle collection, and the
standard packages `std.containers`, `std.strings`, `std.types`, and `std.time`.
The remaining decorators, further standard
libraries, complete module handling, and async remain pending. Maps, deep copies,
and loop control are implemented. `:=` is excluded.

Function parameters now use commas: `func add(int a, int b) => a + b;`. The parser accepts this form and rejects semicolons inside parameter lists, including after the last parameter. The decorator package is `std.Decorator`; `@Decorator.private` and `@Decorator.subclass` are implemented.

It includes:
- a lexer (`src/lexer.rs`)
- AST definitions (`src/ast.rs`)
- a handwritten parser (`src/parser.rs`); `src/grammar.pest` is an unused reference grammar
- a name resolver (`src/resolve.rs`)
- a tree-walking interpreter (`src/interp.rs`), runtime values (`src/value.rs`), and the package loader (`src/package.rs`)
- the standard library: Marslang packages in `std/*.mars` and native Rust packages in `std/rs/*.rs`, registered at build time by `build.rs`
- CLI executable named `marslang` (`src/main.rs`)

## Build

```bash
cargo build
```

## Run a `.mars` file

```bash
cargo run -- hello.mars
# or
cargo run -- run hello.mars
```

`marslang check file.mars` parses and resolves a program without running it.
Syntax errors, unknown names, and fixed-binding reassignment are reported before
anything runs. Runtime errors print `error: <Kind>: <message>` and exit with status 1.

## Current release coverage

Implemented:
- lexical binding resolution: bare assignment updates the nearest binding or declares a local; annotations and `cold` declare explicitly
- `fixed` bindings and recursively immutable containers; direct fixed-copy propagation and `.copy()` for independent mutable deep copies
- constant-expression substitution for hot variables; runtime type checks, signed 32/64-bit integer bounds, and identity-based collection equality
- `func` block and expression forms
- `family` definitions with optional inheritance and `init` -> constructor mapping
- `ret`, assignment, function calls, member access
- `if/elif/else`, `repeat`, `while`, two- and three-part `for`, `break`, and `continue`
- `takepkg` packages (similar to Python's): bundled standard packages (`takepkg std.math;`), package directories with `init.mars`, module files (`takepkg shapes.circle;`), relative imports (`takepkg .sibling;`, `takepkg ..parent;`), and optional aliases
- built-in `arr`, `set`, `pair`, `map`/`dict`; legacy `a`/`s`/`p` remain compatibility spellings
- array operations, collection `.len()`/`.is_empty()`/`.has()`, insertion-ordered map keys and snapshot iteration
- `out`, `slout`, `in`, and `inln`
- `fasle` accepted as `false`

Still intentionally limited in this initial release:
- no full static type checker yet
- no bytecode or native backend yet (programs run in a tree-walking interpreter)
- no complete `match`, hot functions, type aliases, or tagged-variant syntax yet
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
`👨‍👩‍👧‍👦` each count as one character.
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

The REPL keeps a source buffer and re-runs it after each line, showing only the output the new line produced. Lines that fail to compile or raise a runtime error are discarded. Use `:show`, `:reset`, and `:exit`.

## Packages

`takepkg` loads standard packages (`std.*`, built into `marslang`) and package files
next to the main program; see [Packages](docs/api/language.md#packages).

Planned (not implemented yet): a per-user package directory, such as
`C:\Users\<user>\marslang_pkgs` on Windows, where an installer will put third-party
packages so any program can `takepkg` them. The installer and the search order between
that directory and the program's own directory are future work.

## Building a Windows `.exe`

From Windows (or with a Windows target toolchain installed), build:

```bash
cargo build --release
```

The executable will be at `target/release/marslang.exe` on Windows.
