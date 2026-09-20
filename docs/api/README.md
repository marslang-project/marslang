# Marslang language and API reference

This reference describes the Rust interpreter as of `rs-0.8.1`,
including dynamic numeric type tracking and the expanded `std.math` package.
The source extension is `.mars`; earlier releases used `.mrs`.
Use the matching source checkout for the filenames documented here.
These API documents and release notes are eligible for Git tracking. Discussion
notes elsewhere under `docs/` remain local.

## Reference

- [Language syntax](language.md): variables, functions, families, and control flow.
- [Strings and slicing](strings.md): Unicode characters, reversal, and strict bounds.
- [Collections](collections.md): arrays, sets, pairs, maps, and copying.
- [Built-ins and errors](builtins.md): input/output, conversions, and runtime errors.
- [std.math](std-math.md): 51 math functions and six constants, written in Marslang on native float primitives.
- [std.containers](std-containers.md): stack, queue, deque, and priority queue.
- [std.strings](std-strings.md): splitting, searching, trimming, case, and padding.
- [std.types](std-types.md): a value's kind, family checks.
- [std.time](std-time.md): clocks and sleeping.
- [std.Decorator](std-decorator.md): `@Decorator.private` and `@Decorator.subclass` methods.

## Run

Install Rust. Run from the repository root:

```sh
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
