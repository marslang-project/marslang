# marslang

Marslang v0.1 is a small experimental language that compiles `.mrs` source files into Python.

## What is implemented

This repository now contains:

- A lexer that supports Marslang keywords, operators, comments, strings, multiline strings, and punctuation.
- A recursive-descent parser that builds an AST for variables, functions, families, imports, conditionals, loops, match blocks, and error handling.
- A Python code generator that transpiles Marslang into runnable Python and includes a runtime shim for Marslang-specific built-ins and data structures.
- A `compiler` CLI that writes the generated Python file and can optionally run it.

## Supported Marslang v0.1 features

The implementation covers the core language described in the prompt, including:

- `fixed`, `hot`, and `cold` variable modifiers.
- Optional type annotations in the form `name (type) = value;`.
- `func` declarations, single-expression functions via `=>`, and `family` declarations.
- `ret`, `if` / `elif` / `else`, `repeat`, `for`, `match`, and `run` / `handle` / `now_do`.
- `out`, `slout`, `in()`, `inln()`, `err()`, and built-in collection constructors `a(...)`, `s(...)`, `p(...)`.
- Arrays, sets, pairs, type-restricted arrays/sets, and helper methods such as `.add()`, `.iget()`, `.asort()`, `.slice()`, and more.
- `takepkg module;` and `takepkg module = alias;` imports.
- Hot constant inlining for compile-time constant variables and hot single-expression functions.

## Example

```marslang
takepkg math = mt;

fixed hot pi (float) = 3.14159;
func area(float r;) => pi * r * r;

family Circle{
    func init(float r;){
        me.r = r;
    }

    func size(){
        ret area(me.r);
    }
}

func m{
    nums (array[int]) = a(3,1,4,1,5);
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

This will emit `examples/hello.py` and optionally execute it.

## Project layout

- `marslang/lexer.py`: tokenizer.
- `marslang/parser.py`: AST builder.
- `marslang/ast.py`: AST node definitions.
- `marslang/codegen.py`: Python transpiler.
- `marslang/runtime.py`: Marslang runtime helpers.
- `marslang/cli.py`: command-line entrypoint.
- `tests/test_compiler.py`: regression tests.
