# Marslang

Marslang is an experimental programming language with `.mrs` source files and a Python-hosted VM backend. The current repository implements **Marslang v0.4**, which keeps the v0.2+ language shape while hardening parser/runtime behavior, improving CLI error handling, and expanding documentation and test coverage.

## What is new in v0.4

- The parser has been hardened: unsafe lookahead paths were removed, incomplete parsing refactors were cleaned up, and index syntax like `nums[0]` is now fully parsed.
- Runtime semantics are more complete: union types are enforced, indexed reads/writes work, and `and/or` now behaves like a real inclusive boolean-or instead of silently acting like `and`.
- The CLI now reports runtime failures from `--run` as clean `marslang runtime error: ...` messages rather than dumping raw Python tracebacks.
- The test suite now covers parser edge cases, lexer failures, runtime type enforcement, CLI runtime error UX, and the verbose compiler path.
- Documentation has been refreshed for v0.4, including compatibility notes and the legacy `fasle` alias.

## Marslang at a glance

Marslang currently supports:

- `fixed`, `hot`, and `cold` modifiers.
- Optional type syntax such as `name (type) = value;`.
- `func` declarations, hot inline functions, and `family` declarations.
- `ret`, `if`, `elif`, `else`, `repeat`, `for`, `match`, and `run` / `handle` / `then`.
- Built-ins such as `out`, `slout`, `in()`, `inln()`, `err()`, and constructors `arr(...)`, `set(...)`, and `pair(...)`.
- Array / set / pair type restrictions and union types such as `[int, string]`.
- Index syntax like `arr[0]` in addition to collection helpers such as `.iget()`.
- A compiler that emits a Python file containing a serialized Marslang VM program and a runner stub.

## Quickstart

### 1. Write a Marslang program

```marslang
fixed hot pi (float) = 3.14159;
hot func area(float r;) => pi * r * r;

family Circle{
    func init(float r;){
        me.r = r;
    }

    func size(){
        ret area(me.r);
    }
}

func m{
    nums (array[int]) = arr(3,1,4,1,5);
    nums.asort();
    out(nums.iget(0));

    c = Circle(5);
    out(c.size());
}
```

### 2. Compile it

```bash
python3 -m marslang.cli examples/hello.mrs
```

### 3. Compile with verbose output

```bash
python3 -m marslang.cli examples/hello.mrs --verbose
```

### 4. Compile and run

```bash
python3 -m marslang.cli examples/hello.mrs --run
```

### 5. Use the wrapper script

```bash
./bin/compiler examples/hello.mrs --run --verbose
```

## Documentation map

Detailed docs live in `docs/`:

- [`docs/quickstart.md`](docs/quickstart.md): first-run guide and common workflows.
- [`docs/language-reference.md`](docs/language-reference.md): Marslang syntax and semantics reference.
- [`docs/compiler-cli.md`](docs/compiler-cli.md): CLI flags, verbose mode, runtime error UX, and compiler API details.
- [`docs/runtime-api.md`](docs/runtime-api.md): runtime VM, collection types, type enforcement, and execution model.

## Project layout

- `marslang/lexer.py`: tokenizer.
- `marslang/parser.py`: recursive-descent parser.
- `marslang/ast.py`: AST node definitions.
- `marslang/codegen.py`: VM program serializer / code generator.
- `marslang/compiler.py`: compile APIs and compilation result helpers.
- `marslang/runtime.py`: VM executor and Marslang runtime library.
- `marslang/cli.py`: command-line compiler.
- `tests/test_compiler.py`: integration and CLI tests.
- `tests/test_parser_runtime_edges.py`: parser, lexer, and runtime edge-case tests.

## Compatibility note

Marslang still accepts the legacy typo `fasle` as a deprecated compatibility alias for `false`, because earlier language notes explicitly allowed it. The docs now call this out clearly so it is no longer surprising.

## Status

Marslang is still alpha-stage. The current implementation prioritizes correctness, debuggability, and iteration speed over optimization.
