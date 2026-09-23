# rs-0.14.0 — JSON, the environment, and code points

> No binaries were published for rs-0.14.0: its Windows build failed a test.
> Everything below shipped in [rs-0.14.1](rs-0.14.1.md).

Programs can now read and write JSON with [std.json](../api/std-json.md), look
at the machine they run on with [std.os](../api/std-os.md) and at the
interpreter running them with [std.sys](../api/std-sys.md), and convert between
characters and their Unicode numbers with the new built-ins `ord` and `chr`.

## std.json

```mars
takepkg std.json;

func m{
    data = json.parse("{\"name\": \"Ada\", \"tags\": [\"math\", \"code\"], \"age\": 36}");
    out(data.get("name"), data.get("tags")[1]);   // Ada code

    data.set("age", 37);
    out(json.stringify(data));   // {"name":"Ada","tags":["math","code"],"age":37}
}
```

`parse`, `stringify`, `pretty`, and `indented`. Objects become maps that keep
their key order. Whole numbers follow the rule for literals, so `3000000000` is
a `longint`, and floats are written so that `1.0` reads back as a float.

Reading is strict, as RFC 8259 says, and every mistake is a `SyntaxError` that
says where:

```text
SyntaxError: JSON line 3, column 3: expected a value, found 'o'
SyntaxError: JSON line 1, column 7: a comma before '}': JSON allows no trailing comma
```

Values JSON cannot hold raise and say what to write instead: sets and pairs,
family instances, infinity and NaN, maps with keys that are not strings, and a
container that holds itself.

Reading and writing are native Rust: a 1.9 MB document of 15,000 records reads
in about 70 ms and writes back in under 20 ms. Reading JSON one Marslang
character at a time would also have been wrong, because a quote written
directly before a combining accent joins with it into one character.

## std.os and std.sys

```mars
takepkg std.os;
takepkg std.sys;

func m{
    out(os.platform(), os.arch());          // windows x86_64
    out(os.env_or("EDITOR", "nano"));
    out(sys.VERSION, sys.INT_MAX);          // rs-0.14.0 2147483647
}
```

`std.os` is the machine: `env`, `env_or`, `environment`, `cwd`, `home`,
`platform`, `arch`, `pid`, and the path separator `SEP`. `std.sys` is the
interpreter: `VERSION`, the ranges of `int` and `longint`, `MAX_SAFE`,
`executable()`, `package_dir()`, and `standard_packages()`. A program's
arguments and exit status stay in [std.cli](../api/std-cli.md).

## ord and chr

```mars
out(ord("a"), ord("中"), ord("😀"));   // 97 20013 128512
out(chr(97), chr(ord("A") + 2));      // a C
```

A character that is several code points, such as `e` followed by a combining
accent, has no single number, so `ord` refuses it and names the parts:

```text
RangeError: ord needs one code point, but "é" is 2 (U+0065 U+0301); take ord of each part
```

`chr` refuses numbers outside Unicode and the surrogate halves `55296` to
`57343`. See [characters and code points](../api/builtins.md#characters-and-code-points).

## Validation

`cargo test` passes 134 tests (13 unit, 120 execution, 1 memory) on Windows
Rust and on WSL-native Rust. Every example on the new documentation pages was
run against this build.
