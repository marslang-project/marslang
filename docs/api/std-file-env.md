# std.file.env

`.env` files: one `KEY=value` line per setting, as Docker, Compose, and most web
frameworks read them.

```mars
takepkg std.file;
takepkg std.file.env;

func m{
    settings = env.read(file.here(".env"));
    out(settings.get("PORT"));
}
```

```text
# .env
PORT=8080
NAME="My App"   # shown in the title
export DEBUG=true
```

| Function | Result |
| --- | --- |
| `read(path)`, `parse(text)` | A map from name to value |
| `write(path, values)`, `format(values)` | A map as `KEY=value` lines |

## Reading

- Blank lines and `#` comments are skipped, and `export` before a key is allowed.
- An unquoted value runs to the end of the line, or to a `#` after a space:
  `URL=http://host/#top` keeps its `#`.
- A double-quoted value may span lines and takes `\n`, `\t`, `\"`, and `\\`
  escapes; a single-quoted one is taken exactly as written.
- Every value is a string: `PORT=8080` gives `"8080"`; convert with `int()`.
- `$NAME` is not expanded; it stays `$NAME`.
- A key given twice keeps its first position and its last value.

Mistakes raise `SyntaxError` with the line, and the file when there is one:

```text
SyntaxError: .env: env line 4: expected KEY=value
SyntaxError: env line 1: "1BAD" is not a variable name: letters, digits, and _, not starting with a digit
```

## Writing

Plain words are written as they are. Other values are single-quoted, which
every reader takes exactly, so `$` and `#` stay literal; a value holding a
single quote or a line break is double-quoted with escapes instead. Either way
it reads back the same. Numbers and booleans are written as `out` prints them.

Keys must be names: letters, digits, and `_`, not starting with a digit.

`std.file.env` reads and writes files; it does not change the program's
environment. To read the environment itself, use [std.os](std-os.md).

The wrapper is written in Marslang ([std/file/env.mars](../../std/file/env.mars))
over the native package [std/rs/config.rs](../../std/rs/config.rs).
