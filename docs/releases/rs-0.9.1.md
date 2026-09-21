# rs-0.9.1 — Line numbers in compile errors

A patch release: syntax and import errors now say which line they are on, so an
editor can put the error where it belongs.

## Where an error is

```text
error: line 9: expected expression, got End
error: line 2: package 'shapes.circle' not found: looked for ...
```

Statements are parsed from normalized text, where one source line may hold
several statements and one statement may span several lines, so positions were
lost before parsing began. Each normalized piece now keeps the source line it
started on, and a statement that fails reports that line, however deeply it is
nested in functions, methods, loops, or `run` blocks.

A `takepkg` keeps its line as well, so a misspelled package, a missing one, a
circular import, or an error inside an imported package is reported against the
import that caused it.

Name resolution does not have positions yet: `error: unknown name 'nope'` says
what is missing but not where. Runtime errors are unchanged.

## Editor support

[vscode-marslang](https://github.com/marslang-project/vscode-marslang) gives
Visual Studio Code highlighting for `.mars` files and shows what
`marslang check` reports as a diagnostic on the reported line. With this
release, those diagnostics land on the right line; an error without a position
is placed on the name it quotes.

## Validation

`cargo test` passes 112 tests (13 unit, 98 execution, 1 memory) on Windows Rust
and on WSL-native Rust. The extension's tests run this interpreter over broken
programs and check the line each error is reported on.
