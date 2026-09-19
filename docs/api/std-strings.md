# std.strings

```mars
takepkg std.strings;

func m{
    words = strings.split_whitespace("  red green  blue ");
    out(strings.join(words, ","));          // red,green,blue
    out(strings.pad_start("7", 3, "0"));    // 007
}
```

Positions and widths count characters (grapheme clusters), the same unit as
`len()` and `lenslice()`. Searching, splitting, and replacing match whole characters
only: a match must start and end at character boundaries, so a position from
`find` always works with `lenslice`. A combining accent inside a character is not
found on its own, and `"
"` is one character, so `split(text, "
")` does not
split Windows line endings; use `lines(text)` for those. The package is named `strings` so that it does not hide
the built-in `string(value)` conversion.

| Function | Result |
| --- | --- |
| `split(text, separator)` | Array of the parts between separators; an empty separator raises `RangeError` |
| `split_whitespace(text)` | Array of the words between runs of whitespace |
| `lines(text)` | Array of lines, splitting at `\n` or `\r\n` |
| `join(items, separator)` | One string from an array of strings |
| `trim(text)` / `trim_start(text)` / `trim_end(text)` | Text without surrounding whitespace |
| `starts_with(text, prefix)` / `ends_with(text, suffix)` / `contains(text, part)` | Boolean |
| `find(text, part)` / `rfind(text, part)` | Character position of the first/last occurrence, or -1 |
| `replace(text, old, new)` | Every occurrence replaced; an empty `old` raises `RangeError` |
| `upper(text)` / `lower(text)` | Unicode case conversion (`straße` becomes `STRASSE`) |
| `repeated(text, count)` | `text` repeated `count` times |
| `pad_start(text, width, fill)` / `pad_end(text, width, fill)` | Padded to `width` characters with a one-character `fill` |

The Marslang API is in [std/strings.mars](../../std/strings.mars); the text
operations are native (`std/rs/string.rs`).
