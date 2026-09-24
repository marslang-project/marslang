# std.file.toml

[TOML](https://toml.io) files, the configuration format of Cargo, Python
projects (`pyproject.toml`), and many other tools.

```mars
takepkg std.file.toml;

func m{
    project = toml.parse("[package]\nname = \"demo\"\nversion = \"1.0\"\n");
    out(project.get("package").get("name"));   // demo

    project.get("package").set("version", "1.1");
    out(toml.format(project));
}
```

```text
demo
[package]
name = "demo"
version = "1.1"
```

| Function | Result |
| --- | --- |
| `read(path)`, `parse(text)` | A map, keeping the order of its keys |
| `write(path, values)`, `format(values)` | A map as TOML text |

## Reading

| TOML | Marslang |
| --- | --- |
| table, inline table | `map` |
| array, array of tables | `array` |
| string | `string` |
| integer | `int` when it fits in 32 bits, otherwise `longint` |
| float, `inf`, `nan` | `float` |
| boolean | `bool` |
| date | [`date`](dates.md) |
| date and time with an offset | [`datetime`](dates.md) at that offset |
| local date and time, or a time alone | `string`, such as `"07:32:00"` |

A local date and time has no offset, so it names no moment; it stays text, as
does a time alone. Dates and datetimes are written back as TOML dates; a
datetime in a named zone is written with its offset, since TOML has no zone
names. A duration raises `TypeError`, since TOML has none.

The whole of TOML 1.1 is read: dotted keys, multi-line and literal strings,
hexadecimal, octal, and binary integers, and `_` between digits. Mistakes raise
`SyntaxError` with the line and column:

```text
SyntaxError: Cargo.toml: TOML line 7, column 9: string values must be quoted, expected literal string
```

## Writing

Maps inside become `[tables]` and arrays of maps become `[[arrays of tables]]`.
TOML has no null, so a `null` value raises `TypeError`; leave the key out
instead. The whole document must be a map.

Formatting is not kept: comments and the original layout are lost when a file
is read and written back. Reading, changing a value, and writing is right for
files a program owns; for files people edit by hand, expect the comments to go.

Reading and writing use the [`toml`](https://crates.io/crates/toml) crate. The
wrapper is written in Marslang ([std/file/toml.mars](../../std/file/toml.mars))
over the native package [std/rs/config.rs](../../std/rs/config.rs).
