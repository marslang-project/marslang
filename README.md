# marslang

Marslang v0.2 is an experimental language that compiles `.mrs` source files into a small Python-hosted VM program.

## What changed in v0.2

- `now_do { ... }` is now `then { ... }` for post-`handle` execution.
- Collection constructors are now `arr(...)`, `set(...)`, and `pair(...)`.
- `for` loops use `for(iterator, condition, change_iterator){...}` C++-style semantics with commas instead of semicolons.
- The compiler backend now emits a VM program payload plus a Python runner instead of direct Python source translation.

## What is implemented

This repository contains:

- A lexer that supports Marslang keywords, operators, comments, strings, multiline strings, and punctuation.
- A recursive-descent parser that builds an AST for variables, functions, families, imports, conditionals, loops, match blocks, and error handling.
- A compiler that lowers Marslang AST nodes into a VM program representation embedded in Python.
- A runtime VM that executes compiled Marslang programs and provides Marslang-specific built-ins and data structures.
- A `compiler` CLI that writes the generated Python file and can optionally run it.

## Supported Marslang v0.2 features

The implementation covers the core alpha language, including:

- `fixed`, `hot`, and `cold` variable modifiers.
- Optional type annotations in the form `name (type) = value;`.
- `func` declarations, single-expression functions via `=>`, hot inline functions, and `family` declarations.
- `ret`, `if` / `elif` / `else`, `repeat`, `for`, `match`, and `run` / `handle` / `then`.
- `out`, `slout`, `in()`, `inln()`, `err()`, and built-in collection constructors `arr(...)`, `set(...)`, `pair(...)`.
- Arrays, sets, pairs, type-restricted arrays/sets, and helper methods such as `.add()`, `.iget()`, `.asort()`, `.slice()`, and more.
- `takepkg module;` and `takepkg module = alias;` imports.
- Hot constant inlining for compile-time constant variables and hot single-expression functions.
- A Python-hosted VM runner via `marslang.runtime.execute_program`.

## Example

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

## Usage

Compile a file:

```bash
python3 -m marslang.cli examples/hello.mrs
```

Or use the wrapper script:

```bash
./bin/compiler examples/hello.mrs --run
```

This emits a Python file containing the serialized VM program and runner.

## Project layout

- `marslang/lexer.py`: tokenizer.
- `marslang/parser.py`: AST builder.
- `marslang/ast.py`: AST node definitions.
- `marslang/codegen.py`: VM program compiler.
- `marslang/runtime.py`: VM, runtime helpers, and collection implementations.
- `marslang/cli.py`: command-line entrypoint.
- `tests/test_compiler.py`: regression tests.
