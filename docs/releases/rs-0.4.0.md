# rs-0.4.0

The canonical Marslang source extension is now `.mars`.

## Migration

Rename your `.mrs` source files to `.mars` and update references in commands and
scripts. The bundled files are now `hello.mars`, `stdlib.mars`, and
`docs/api/examples/strings.mars`. Generated output still uses `.js`.

```sh
cargo run -- compile hello.mars -o hello.js
node hello.js
```

The CLI accepts explicitly supplied source paths without enforcing an extension,
so older filenames can still be compiled. CLI help, tests, and current API
documentation use `.mars`. Historical release notes retain their original names.

## Validation

`cargo test`: 55 passing tests (7 unit and 48 integration), none ignored.
The renamed bundled example compiles and runs under Node, printing `3` and
`78.53975`. The renamed Unicode API example is covered by an execution test.

Language behavior and the limitations documented in
[rs-0.3.0](rs-0.3.0.md) are otherwise unchanged.
