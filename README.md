# marslang

Marslang is a Rust implementation of the **marslang rs-0.2.0** compiler.

See the [rs-0.2.0 release notes](docs/releases/rs-0.2.0.md) for the parameter syntax migration and known limitations.

See the [project review and roadmap](docs/PROJECT_REVIEW.md) for known limitations, the core correctness milestones, and the plan to eventually write the compiler in Marslang itself (self-hosting).

The [language contract](docs/LANGUAGE_CORE.md) records the original syntax instructions and their later revisions. It distinguishes specified behavior, deferred implementation, and unresolved semantics. In particular, the canonical constructors are `arr`/`set`/`pair` and cleanup uses `then`; the current implementation and examples below still contain older spellings.

Run `cargo test` for current coverage, including Node execution tests. Run `cargo test --test execution -- --ignored` to exercise documented compiler gaps (expected failures until implemented). Node must be on PATH, or set `MARSLANG_NODE` to its executable path.

See the [feature inventory](docs/LIMITATIONS.md) for approved additions, remaining design choices, and exclusions. The [language contract](docs/LANGUAGE_CORE.md) records the September 16 decisions on decorators, builtin maps/deque, libraries, copying, imports, and loops. These additions await implementation; `:=` is excluded and async is deferred.

Function parameters now use commas: `func add(int a, int b) => a + b;`. The parser accepts this form and rejects semicolons inside parameter lists, including after the last parameter. The decorator package is `std.Decorator`; decorator execution remains pending.

It includes:
- a lexer (`src/lexer.rs`)
- AST definitions (`src/ast.rs`)
- a handwritten parser (`src/parser.rs`); `src/grammar.pest` is an unused reference grammar
- a compiler/transpiler to JavaScript (`src/eval.rs`)
- CLI executable named `marslang` (`src/main.rs`)

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

## Current rs-0.2.0 coverage

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

func area(float r) => pi * r * r;

family Circle{
    func init(float r){
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

The executable will be at `target/release/marslang.exe` on Windows.
