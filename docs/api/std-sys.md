# std.sys

The interpreter running the program: its version, where it is, where installed
packages come from, and the limits of its number kinds.

```mars
takepkg std.sys;

func m{
    out(sys.VERSION);                  // rs-0.14.0
    out(sys.INT_MAX, sys.LONGINT_MAX); // 2147483647 9223372036854775807
    out(sys.standard_packages());      // ["std.algorithms", "std.cli", ...]
}
```

| Function or value | Result |
| --- | --- |
| `VERSION` | The interpreter's version, as `marslang --version` prints it |
| `INT_MIN`, `INT_MAX` | The range of `int`: `-2147483648` to `2147483647` |
| `LONGINT_MIN`, `LONGINT_MAX` | The range of `longint`: `-9223372036854775808` to `9223372036854775807` |
| `MAX_SAFE` | `9007199254740991`, 2^53 - 1: the largest whole number a float holds exactly |
| `executable()` | The path of the `marslang` executable, or `null` when the system does not say |
| `package_dir()` | Where installed packages are imported from, as `marslang pkgs` prints it |
| `standard_packages()` | The name of every standard package, as an array of strings |

`MAX_SAFE` is the edge the interpreter checks `int` arithmetic against: a result
beyond it would lose digits, so it raises `RangeError` and asks for `longint`
instead of rounding silently.

`package_dir()` is `MARSLANG_PKGS` when that is set, and otherwise
`marslang_pkgs` in the home directory; it is `null` only when there is no home
directory to use.

Python's `sys` also holds the program's arguments and exit; in Marslang those are
[std.cli](std-cli.md). The machine the program runs on is [std.os](std-os.md).

The wrapper is written in Marslang ([std/sys.mars](../../std/sys.mars)) over the
native package [std/rs/sys.rs](../../std/rs/sys.rs).
