# std.file.ini

INI files: `key = value` lines grouped under `[section]` headers, as many
desktop programs and Windows settings files use.

```mars
takepkg std.file.ini;

func m{
    config = ini.parse("name = demo\n[server]\nport = 8080\nhost: example.com\n");
    out(config.get("name"), config.get("server").get("port"));   // demo 8080
}
```

| Function | Result |
| --- | --- |
| `read(path)`, `parse(text)` | A map |
| `write(path, values)`, `format(values)` | A map as INI text |

Keys before the first `[section]` are top-level keys of the map, and each
section is a map inside it, the same shape TOML gives:

```text
{"name": "demo", "server": {"port": "8080", "host": "example.com"}}
```

## Reading

- `key = value` and `key: value` both work; the spaces around key and value are removed.
- Lines starting with `;` or `#` are comments. A `;` later on a line is part of the value.
- Every value is a string. INI has no quoting, so quotes stay part of the value.
- A section named twice, a key given twice in one section, and a line that is
  none of these raise `SyntaxError` with the line.

## Writing

Plain values become top-level keys and map values become sections, top-level
keys first. Values are text, numbers, or booleans. INI has one level of
sections, so a map inside a section raises `TypeError`, and a value holding a
line break raises `RangeError`. Keys cannot contain `=`, `:`, brackets, or line
breaks.

The wrapper is written in Marslang ([std/file/ini.mars](../../std/file/ini.mars))
over the native package [std/rs/config.rs](../../std/rs/config.rs).
