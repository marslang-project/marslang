# Marslang

Marslang is an experimental programming language with `.mrs` source files and a Python-hosted VM backend. The current repository implements **Marslang v0.3**, which keeps the v0.2 language syntax and runtime model, while improving the compiler/CLI workflow and adding more complete documentation.

## What is new in v0.3

- The compiler and CLI are now aligned with the VM-based v0.2 backend.
- The CLI supports `-v` / `--verbose` for detailed compilation progress output.
- The compiler exposes richer program-analysis APIs for tooling and tests.
- The documentation has been expanded with quickstart, language reference, compiler CLI, and runtime API guides in `docs/`.

## Marslang at a glance

Marslang currently supports:

- `fixed`, `hot`, and `cold` modifiers.
- Optional type syntax such as `name (type) = value;`.
- `func` declarations, hot inline functions, and `family` declarations.
- `ret`, `if`, `elif`, `else`, `repeat`, `for`, `match`, and `run` / `handle` / `then`.
- Built-ins such as `out`, `slout`, `in()`, `inln()`, `err()`, and constructors `arr(...)`, `set(...)`, and `pair(...)`.
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
- [`docs/compiler-cli.md`](docs/compiler-cli.md): CLI flags, verbose mode, and compiler API details.
- [`docs/runtime-api.md`](docs/runtime-api.md): runtime VM, collection types, and execution model.

## Project layout

- `marslang/lexer.py`: tokenizer.
- `marslang/parser.py`: recursive-descent parser.
- `marslang/ast.py`: AST node definitions.
- `marslang/codegen.py`: VM program serializer / code generator.
- `marslang/compiler.py`: compile APIs and compilation result helpers.
- `marslang/runtime.py`: VM executor and Marslang runtime library.
- `marslang/cli.py`: command-line compiler.
- `tests/test_compiler.py`: regression tests.

## Status

Marslang is still an alpha-stage language. The current implementation focuses on making the language runnable, testable, and hackable rather than fully optimizing every construct.
