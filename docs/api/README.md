# Marslang language and API reference

This reference describes the Rust interpreter as of `rs-0.11.1`,
including dynamic numeric type tracking and the expanded `std.math` package.
The source extension is `.mars`; earlier releases used `.mrs`.
Use the matching source checkout for the filenames documented here.
These API documents and release notes are eligible for Git tracking. Discussion
notes elsewhere under `docs/` remain local.

## Reference

- [Language syntax](language.md): variables, functions, families, and control flow.
- [Grammar](grammar.md): exactly what may be written, and operator precedence.
- [Strings and slicing](strings.md): Unicode characters, reversal, and strict bounds.
- [Collections](collections.md): arrays, sets, pairs, maps, and copying.
- [Built-ins and errors](builtins.md): input/output, conversions, and runtime errors.
- [std.math](std-math.md): 51 math functions and six constants, written in Marslang on native float primitives.
- [std.containers](std-containers.md): stack, queue, deque, and priority queue.
- [std.algorithms](std-algorithms.md): sorting by a key or a comparison, binary search, and the usual collection passes.
- [std.stats](std-stats.md): mean, median, mode, quantiles, spread, and correlation.
- [std.strings](std-strings.md): splitting, searching, trimming, case, and padding.
- [std.types](std-types.md): a value's kind, family checks.
- [std.memory](std-memory.md): addresses, object identity, and the cycle collector.
- [std.file](std-file.md): reading, writing, listing, copying, moving, and removing files and directories.
  - [std.file.path](std-file-path.md): joining and splitting paths, names, and extensions.
  - [std.file.csv](std-file-csv.md): reading and writing CSV tables.
- [std.json](std-json.md): reading and writing JSON, as text and as files.
- [std.os](std-os.md): environment variables, the working and home directories, and the platform.
- [std.sys](std-sys.md): the interpreter's version, number limits, and package directory.
- [std.time](std-time.md): clocks and sleeping.
- [std.Decorator](std-decorator.md): `@Decorator.docstring`, and `@Decorator.private` and `@Decorator.subclass` methods.
- [std.Error](std-error.md): the built-in error families as `Error.TypeError`, `Error.RangeError`, and so on.
- [std.cli](std-cli.md): command-line arguments, a parser for options, and exit statuses.

## Install

Released builds need no Rust toolchain and install into your home directory:

```powershell
irm https://marslang.kevin-z.com/install.ps1 | iex
```

```sh
curl -fsSL https://marslang.kevin-z.com/install.sh | sh
```

Both accept `--use std,ext` (`-Use` in PowerShell) for the package sets to
install; `std` is built into the interpreter, and `ext` is not published yet.
`marslang pkgs` prints where packages you install are imported from.

For editing, [vscode-marslang](https://github.com/marslang-project/vscode-marslang)
gives Visual Studio Code highlighting for `.mars` files and shows what
`marslang check` reports as you save.

## Run

From an installed interpreter, or with Rust from the repository root:

```sh
marslang docs/api/examples/strings.mars
cargo run -- docs/api/examples/strings.mars
```

The [runnable example](examples/strings.mars) prints:

```text
CDE
EDCBA
ABCDE
3
é👨‍👩‍👧‍👦
0
```

Use `cargo test` to run the execution tests, which run programs in the interpreter
and check their output. The example above is also an execution test.

## Current limits

Programs run in a Rust tree-walking interpreter. The language is not self-hosted yet.
Additional standard-library packages, the remaining decorators (`static`, `class`,
`overload`), `match`, and async remain unimplemented. General named arguments
are deferred; only slicing accepts `reverse=`. Proposed packages are not APIs.

The `lex` command uses a separate lexer from the parser. The REPL reruns its
accumulated source rather than retaining a persistent runtime; it shows only new output.
