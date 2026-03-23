# Compiler and CLI Guide

## Command-line usage

Primary interface:

```bash
python3 -m marslang.cli <input.mrs> [options]
```

Wrapper scripts:

Unix-like shells:

```bash
./bin/compiler <input.mrs> [options]
```

Windows `cmd.exe`:

```bat
bin\compiler.bat <input.mrs> [options]
```

## CLI flags

### `-o`, `--output`

Choose the output Python file path.

```bash
python3 -m marslang.cli hello.mrs --output build/hello_vm.py
```

### `--run`

Compile and immediately execute the generated Python runner.

```bash
python3 -m marslang.cli hello.mrs --run
```

### `-v`, `--verbose`

Print detailed compiler progress, including:

- source file path
- token count
- top-level AST node count
- output path
- run stage details when `--run` is used

Example:

```bash
python3 -m marslang.cli hello.mrs --run --verbose
```

## Python API

## `compile_source(source: str) -> str`

Compile Marslang source text into the generated Python runner text.

## `analyze_source(source: str) -> tuple[list[Token], Program]`

Tokenize and parse source without generating emitted Python.

## `compile_source_detailed(source: str, input_path=None) -> CompilationResult`

Return the generated Python source together with tokens and parsed AST information.

## `compile_file(input_path, output_path=None) -> Path`

Compile a file and return the output path.

## `compile_file_detailed(input_path, output_path=None) -> CompilationResult`

Compile a file and return a full compilation result object.

## `CompilationResult`

Fields:

- `input_path`
- `output_path`
- `source`
- `tokens`
- `program`
- `python_source`

Convenience properties:

- `token_count`
- `top_level_count`

## Error handling

The CLI reports compiler and filesystem failures as `marslang compile error: ...` and exits with a non-zero status code. When `--run` is used, runtime failures are reported as `marslang runtime error: ...` so users are not exposed to raw Python stack traces during normal CLI usage.
