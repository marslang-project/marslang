# marslang

Marslang is an interpreted language. This repository is its Rust implementation (**marslang rs-0.11.1**): it parses and checks a `.mars` program, then runs it directly. No JavaScript or Node is involved.

What each release changed is in [docs/releases/](docs/releases/) and on the
[changelog](https://marslang.kevin-z.com/changelog.html).

The development direction is to stabilize the Rust interpreter before self-hosting. Discussion documents under `docs/` stay local; `docs/api/` and `docs/releases/` are available to Git.

Read the [language and API reference](docs/api/README.md) for current syntax and built-in APIs.
The same reference is published at [marslang.kevin-z.com](https://marslang.kevin-z.com); the site
generator lives in [marslang-project/website](https://github.com/marslang-project/website).

Run `cargo test` for current coverage. The execution tests run each program in the interpreter and assert its output or runtime error. Only Rust is required.

Pending work is listed under [current limits](docs/api/README.md#current-limits).

It includes:
- a lexer (`src/lexer.rs`)
- AST definitions (`src/ast.rs`)
- a handwritten parser (`src/parser.rs`); the grammar it accepts is written out in [docs/api/grammar.md](docs/api/grammar.md)
- a name resolver (`src/resolve.rs`)
- a tree-walking interpreter (`src/interp.rs`), runtime values (`src/value.rs`), and the package loader (`src/package.rs`)
- the standard library: Marslang packages in `std/*.mars` and native Rust packages in `std/rs/*.rs`, registered at build time by `build.rs`
- CLI executable named `marslang` (`src/main.rs`)

## Install

A released build installs into your home directory and needs no Rust toolchain:

```powershell
irm https://marslang.kevin-z.com/install.ps1 | iex
```

```bash
curl -fsSL https://marslang.kevin-z.com/install.sh | sh
```

Both scripts live in [install/](install/): they download the archive for your
platform from the release, check it against the release's `SHA256SUMS`, put
`marslang` in a per-user directory, add it to `PATH`, and create the package
directory `marslang_pkgs` in your home directory. `--use std,ext` (`-Use` in
PowerShell) selects package sets; `std` is built into the interpreter and `ext`
is not published yet. The archives are built for Windows, Linux, and macOS by
[.github/workflows/release.yml](.github/workflows/release.yml) when an `rs-*`
tag is pushed.

`marslang pkgs` prints the directory that installed packages are imported from:
`MARSLANG_PKGS`, or `marslang_pkgs` in your home directory. A program's own
directory is always searched first.

## Edit

[vscode-marslang](https://github.com/marslang-project/vscode-marslang) is the
Visual Studio Code extension: `.mars` files become their own language, are
highlighted, and are checked by this interpreter when they are opened and saved,
with each error shown on its line.

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
`marslang symbols file.mars` prints what it declares, and what the packages it
imports export, with types and docstrings, as JSON for editors (`--stdin` reads
the source from standard input).
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

Not yet:
- no static type checker: annotations are checked as the program runs
- no bytecode or native backend: programs run in a tree-walking interpreter
- no `match`, hot functions, type aliases, or tagged variants
- runtime errors do not report a source line yet; compile errors do
- the parser works on normalized statement text; `marslang lex` uses a separate, simpler lexer

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

A package the program's own folder does not have is looked for next in your
package directory, `marslang_pkgs` in your home directory (or `MARSLANG_PKGS`),
which the installers create; `marslang pkgs` prints where it is. See
[your package directory](docs/api/language.md#your-package-directory).

## Building a Windows `.exe`

From Windows (or with a Windows target toolchain installed), build:

```bash
cargo build --release
```

The executable will be at `target/release/marslang.exe` on Windows.

## License

Marslang is **source-available** under the [Marslang Source License](LICENSE.md),
not an open source license. In short:

- Use it, and share unmodified copies free of charge, for any noncommercial purpose.
- Change it only to prepare a contribution, such as a pull request.
- Programs you write in Marslang are yours, to use and license however you like.
- Commercial use, and anything else the license does not cover, needs the
  author's written permission: open an issue to ask.
