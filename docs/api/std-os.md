# std.os

The machine and the process a program runs in: environment variables, the
working directory, the home directory, and which platform this is.

```mars
takepkg std.os;

func m{
    out(os.platform(), os.arch());          // windows x86_64
    editor = os.env_or("EDITOR", "nano");
    out("editing with", editor, "in", os.cwd());
}
```

| Function or value | Result |
| --- | --- |
| `env(name)` | The value of an environment variable, or `null` when it is not set |
| `env_or(name, fallback)` | The value, or `fallback` when it is not set |
| `environment()` | Every environment variable, as a map sorted by name |
| `cwd()` | The working directory, which relative paths are read from |
| `home()` | The user's home directory, or `null` when the environment does not say |
| `platform()` | `"windows"`, `"linux"`, or `"macos"`, otherwise the name Rust gives it |
| `arch()` | The processor architecture, such as `"x86_64"` or `"aarch64"` |
| `pid()` | The process ID of the running program |
| `SEP` | The path separator: `"\\"` on Windows, `"/"` elsewhere |

Environment variable names are case-sensitive, except on Windows, where `Path`
and `PATH` are one variable. `env` follows the platform's rule, and on Windows
`environment()` spells every name in upper case, as Python's `os.environ` does,
so `os.environment().get("PATH")` works everywhere. A name that is empty or
contains `=` raises `RangeError`. A value that is not valid Unicode is
read with replacement characters rather than raising.

`std.os` reads the environment and does not change it. Files and directories
belong to [std.file](std-file.md). The program's own arguments and exit
status are [std.cli](std-cli.md), and the interpreter running it is
[std.sys](std-sys.md).

The wrapper is written in Marslang ([std/os.mars](../../std/os.mars)) over the
native package [std/rs/os.rs](../../std/rs/os.rs).
