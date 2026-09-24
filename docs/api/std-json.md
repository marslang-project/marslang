# std.json

Reading and writing [JSON](https://www.rfc-editor.org/rfc/rfc8259), the text
format most programs and web services exchange data in.

```mars
takepkg std.json;

func m{
    data = json.parse("{\"name\": \"Ada\", \"tags\": [\"math\", \"code\"], \"age\": 36}");
    out(data.get("name"), data.get("tags")[1]);   // Ada code

    data.set("age", 37);
    out(json.stringify(data));   // {"name":"Ada","tags":["math","code"],"age":37}
}
```

| Function | Result |
| --- | --- |
| `parse(text)` | The value the JSON text describes |
| `stringify(value)` | The value as JSON on one line, with no spaces |
| `pretty(value)` | The value as JSON across lines, two spaces per level |
| `indented(value, spaces)` | As `pretty`, with `1` to `16` spaces per level |
| `load(path)` | Reads a JSON file and parses it |
| `save(path, value)` | Writes a value to a file as pretty JSON |

## Reading

| JSON | Marslang |
| --- | --- |
| object | `map`, keeping the order of its keys |
| array | `array` |
| string | `string` |
| whole number | `int` when it fits in 32 bits, otherwise `longint` |
| number with `.` or an exponent | `float` |
| `true`, `false`, `null` | themselves |

Whole numbers follow the rule for number literals in a program, so `3000000000`
is a `longint`. A whole number beyond `longint` raises `RangeError` rather than
silently losing digits; a service that sends such IDs usually sends them as
strings for that reason. A key that appears twice keeps its first position and
its last value.

Reading is strict, as RFC 8259 says: no comments, no trailing commas, no single
quotes, no `NaN` or `Infinity`, and no leading zeros. Text that is not JSON
raises `SyntaxError` with where the problem is and what was expected:

```text
SyntaxError: JSON line 3, column 3: expected a value, found 'o'
SyntaxError: JSON line 1, column 7: a comma before '}': JSON allows no trailing comma
```

Nesting deeper than 512 levels is refused with a `SyntaxError` rather than
risking the interpreter's stack.

## Writing

```mars
out(json.pretty(json.parse("{\"a\": [1, {}], \"b\": []}")));
```

```text
{
  "a": [
    1,
    {}
  ],
  "b": []
}
```

Floats are written as the shortest text that reads back as the same number, and
always with a `.` or an exponent, so `1.0` is written `1.0` and stays a float
when read back. Strings are written as they are, with quotes, backslashes, and
control characters escaped; other characters, such as `é` or `😀`, are written
directly.

Some values have no JSON form and raise instead of being guessed at:

| Value | Raises | Write instead |
| --- | --- | --- |
| a map with a key that is not a string | `TypeError` | a map with string keys |
| a `set` or a `pair` | `TypeError` | an array |
| a family instance | `TypeError` | a map of the fields to write |
| a function or a package | `TypeError` | — |
| infinity or NaN | `RangeError` | a string, or `null` |
| a container that holds itself | `TypeError` | — |

The same array can appear twice in a value; only one that contains itself is
refused.

## Files

```mars
takepkg std.json;

func m{
    settings = json.load("settings.json");
    settings.set("theme", "dark");
    json.save("settings.json", settings);
}
```

`load` reads through [std.file](std-file.md), so a missing file raises
`file.NotFoundError`, and a parse error names the file:

```text
SyntaxError: settings.json: JSON line 3, column 3: expected a value, found 'o'
```

`save` writes pretty JSON, two spaces per level, ending in a newline, and is all
or nothing like `file.write`: a crash while saving leaves the old file whole.

## Speed

Reading and writing are native: a 1.9 MB document of 15,000 records is read in
about 70 ms and written back in under 20 ms by a release build. Doing it in
Marslang itself would also be wrong, not just slower: Marslang's strings count
whole characters, and a quote written directly before a combining accent would
join with it into one.

The wrapper is written in Marslang ([std/json.mars](../../std/json.mars)) over
the native package [std/rs/json.rs](../../std/rs/json.rs).
