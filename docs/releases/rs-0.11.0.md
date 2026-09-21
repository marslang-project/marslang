# rs-0.11.0 — Docstrings, triple-quoted strings, and `marslang symbols`

Functions, families, and methods can carry documentation, strings can span
lines, and the interpreter can describe a program to an editor.

## Triple-quoted strings

```mars
banner = """Marslang
  says "hello"
  to\tyou""";
```

A string in triple double quotes keeps its line breaks, indentation, and inner
`"` characters; escapes still work. Lines after it keep their numbers, so an
error further down still names the right line.

## @Decorator.docstring

```mars
takepkg std.Decorator;

@Decorator.docstring("""
    A circle, measured from its radius.

    area() uses pi to five places.
""")
family Circle{
    @Decorator.docstring("The area inside the circle.")
    func area() => 3.14159 * me.r * me.r;
}
```

Decorators now work above top-level functions and families as well as methods.
`@Decorator.docstring` takes one string and removes the indentation its lines
share, so it can be indented with the code. It changes nothing at runtime.
`@Decorator.private` and `@Decorator.subclass` still apply to methods only, and
say so when written elsewhere.

## marslang symbols

`marslang symbols file.mars` prints, as JSON, every function, family, and method
with its parameters, source lines, and docstring, and every variable with its
scope and type: the annotation when there is one, otherwise the type its first
value shows, marked as inferred. `--stdin` reads the source from standard input,
so an editor can describe unsaved text. The
[VS Code extension](https://github.com/marslang-project/vscode-marslang) uses it
to show types and docstrings on hover.

## Changed

- Errors from reading the source now start with the line, like other compile
  errors: `line 2: unterminated string starting at column 9` instead of
  `unterminated string at 2:9`, and an unclosed string is reported where it
  opens rather than where the file ends.
- The decorator marker list is `docstring, private, subclass, static, class,
  overload`.

## Validation

`cargo test` passes 116 tests (13 unit, 102 execution, 1 memory) on Windows Rust
and on WSL-native Rust.
