# Marslang language and API reference

This reference describes the Rust-to-JavaScript compiler release `rs-0.3.0`.
Use the matching source checkout for the APIs documented here.
These API documents and release notes are eligible for Git tracking. Discussion
notes elsewhere under `docs/` remain local.

## Reference

- [Language syntax](language.md): variables, functions, families, and control flow.
- [Strings and slicing](strings.md): Unicode characters, reversal, and strict bounds.
- [Collections](collections.md): arrays, sets, pairs, maps, and copying.
- [Built-ins and errors](builtins.md): input/output, conversions, and runtime errors.

## Compile and run

Install Rust and Node.js. String operations require Node with `Intl.Segmenter`.
Run from the repository root:

```sh
cargo run -- compile docs/api/examples/strings.mrs -o target/api-strings.js
node target/api-strings.js
```

The [runnable example](examples/strings.mrs) prints:

```text
CDE
EDCBA
ABCDE
3
é👨‍👩‍👧‍👦
0
```

Use `cargo test` to run the Rust and Node execution tests. Set `MARSLANG_NODE` to
the Node executable if it is not on PATH. The example above is also an execution test.

## Current limits

The compiler emits JavaScript for Node. The language is not self-hosted yet.
Complete modules/exports, standard-library packages, decorators, deque,
`match`, `run/handle/then`, and async remain unimplemented. General named arguments
are deferred; only slicing accepts `reverse=`. Proposed packages are not APIs.

The `lex` command uses a separate lexer from compilation. The REPL reruns its
accumulated source rather than retaining a persistent runtime.
