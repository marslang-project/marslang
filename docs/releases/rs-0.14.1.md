# rs-0.14.1 — rs-0.14.0, released

rs-0.14.1 is the release of everything in [rs-0.14.0](rs-0.14.0.md):
[std.json](../api/std-json.md), [std.os](../api/std-os.md),
[std.sys](../api/std-sys.md), and the built-ins `ord` and `chr`. rs-0.14.0 was
tagged, but its Windows build failed a test, so no binaries were published for
it and the installers stayed on rs-0.13.0. Install rs-0.14.1.

## Environment names on Windows

The failing test found a real difference between platforms. Windows ignores
case in environment variable names, and some Windows systems spell `PATH` as
`Path`. `os.env("PATH")` found it either way, but `os.environment()` listed it
under `Path`, so `os.environment().get("PATH")` was `null` on those machines.

On Windows, `environment()` now spells every name in upper case, as Python's
`os.environ` does, so the same program works on every platform:

```mars
takepkg std.os;

func m{
    out(os.environment().get("PATH") != null);   // true on Windows, Linux, and macOS
}
```

On Linux and macOS names keep their spelling, because there `Path` and `PATH`
are two different variables.

## Validation

`cargo test` passes 134 tests (13 unit, 120 execution, 1 memory) on Windows
Rust, on WSL-native Rust, and on GitHub's Windows and Linux runners. The test now
sets a mixed-case variable itself, so it no longer depends on how the machine
running it spells `PATH`.
