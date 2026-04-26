# marslang

Marslang is a Rust implementation of the **marslang rs-0.1.3** compiler.

It includes:
- a lexer (`src/lexer.rs`)
- AST definitions (`src/ast.rs`)
- a parser (Pest grammar + parser in `src/grammar.pest` and `src/parser.rs`)
- a compiler/transpiler to JavaScript (`src/eval.rs`)
- CLI executable named `compiler` (`src/main.rs`)

## Build

```bash
cargo build
```

## Compile a `.mrs` file

```bash
cargo run -- hello.mrs
# or
cargo run -- compile hello.mrs -o hello.js
```

Then run the generated JavaScript:

```bash
node hello.js
```

## Current rs-0.1.3 coverage

Implemented in this version:
- `hot`, `cold`, `fixed` variable declarations (`hot`/`fixed` compile to `const`)
- type annotation syntax like `x (int) = 1;` (parsed and stored)
- `func` block and expression forms
- `family` definitions with optional inheritance and `init` -> constructor mapping
- `ret`, assignment, function calls, member access
- `if/elif/else`, `repeat`
- `takepkg module;` and `takepkg module = alias;`
- built-in runtime mappings for `out`, `slout`, `in`, `inln`, `a`, `s`, `p`
- `fasle` accepted as `false`

Still intentionally limited in this initial release:
- no full static type checker yet
- no dedicated bytecode/native backend yet (current backend is JS)
- no complete `match`, `for`, and `run/handle/now_do` lowering yet

## Example

```mars
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
    out(nums);

    c = Circle(5);
    out(c.size());
}
```

A starter marslang stdlib is included at `stdlib.mrs`.


## REPL

```bash
cargo run -- repl
```

The REPL keeps a stateful source buffer and re-runs it after each line. Use `:show`, `:reset`, and `:exit`.

## Building a Windows `.exe`

From Windows (or with a Windows target toolchain installed), build:

```bash
cargo build --release
```

The executable will be at `target/release/compiler.exe` on Windows.
