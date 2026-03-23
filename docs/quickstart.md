# Marslang Quickstart

## Overview

Marslang programs are written in `.mrs` files and compiled into Python files that embed a serialized VM program plus a runner. You can then execute the generated Python file directly or ask the CLI to run it for you. As of v0.4, the parser/runtime path is also hardened for index syntax, union types, and cleaner CLI runtime failures.

## Installation / setup

You can work with the repo directly:

```bash
python3 -m pytest -q
python3 -m marslang.cli examples/hello.mrs --run
```

Or install the package in editable mode:

```bash
python3 -m pip install -e .
compiler examples/hello.mrs --run
```

On Windows Command Prompt you can also use:

```bat
bin\compiler.bat examples\hello.mrs --run
```

## First program

Create `hello.mrs`:

```marslang
func m{
    out("Hello from Marslang");
}
```

Compile it:

```bash
python3 -m marslang.cli hello.mrs
```

Run it immediately:

```bash
python3 -m marslang.cli hello.mrs --run
```

## Verbose compilation

Use `-v` or `--verbose` to show extra information such as token count, top-level node count, output path, and run stage details. Runtime failures triggered through `--run` are now reported cleanly at the CLI boundary.

```bash
python3 -m marslang.cli hello.mrs --run --verbose
```

## Typical workflow

1. Edit a `.mrs` file.
2. Compile it with `compiler your_file.mrs`.
3. Use `--verbose` when debugging parser/compiler behavior.
4. On Unix-like systems use `./bin/compiler`; on Windows use `bin\compiler.bat`.
5. Use `--run` for quick iteration.
6. Inspect the generated `.py` file if you want to see the serialized VM program payload.

## Example program using v0.2+ / v0.4-hardened syntax

```marslang
hot x (int) = 5;

func m{
    nums (array[int]) = arr(x, 6, 7);
    for(i = 0, i < 3, i++){
        out(nums[i]);
    }
}
```
