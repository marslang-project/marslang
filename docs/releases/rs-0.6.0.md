# rs-0.6.0 — Rust interpreter and packages

Marslang is now an interpreted language. The JavaScript backend is removed:
`marslang file.mars` parses, checks, and runs a program directly in Rust. Node is no
longer needed to run programs or tests.

## Interpreter

- `src/interp.rs` is a tree-walking interpreter over the resolved syntax tree;
  `src/value.rs` holds runtime values. The previous `runtime.js.inc` semantics are
  preserved: numeric kinds, checked integers, fixed containers, deep copies,
  grapheme-based strings, slicing, and the error kinds.
- CLI: `marslang file.mars` or `marslang run file.mars` runs a program;
  `marslang check file.mars` reports compile errors without running;
  `marslang --version`. The `compile ... -o out.js` command is removed.
- Runtime errors print `error: <Kind>: <message>` and exit with status 1.
- Calls nested deeper than 10,000 levels raise `RangeError: maximum call depth exceeded`.
- The REPL shows only the output produced by each new line and discards lines that
  fail to compile or run.
- String segmentation uses the `unicode-segmentation` crate.

Behavior changes from rs-0.5.0:

- Numbers compare by exact value across every kind: `1 == longint(1)` and
  `longint(5) < 10` are true, and equal numbers share set elements and map keys.
- Calling a function, method, or built-in with the wrong number of arguments raises
  `TypeError`; reading a missing family field raises `TypeError`.
- A block function without `ret` returns `null`.
- Containers print readably, such as `[1, 2]`, `{"k": 1}`, and `Box { v: 3 }`. The
  display format is still not a stable serialization format.
- Integer overflow of 32-bit operands reports `int overflow`.

## Packages

`takepkg` is a package system, similar to Python's:

- `takepkg std.math;` loads a standard package built into `marslang`.
- `takepkg util;` loads `util/init.mars` (a package directory) or `util.mars` from
  the main program's directory; `takepkg shapes.circle;` maps to subdirectories.
- `takepkg .sibling;` and `takepkg ..parent;` are relative imports inside packages.
- Importing `a.b` runs `a/init.mars` first. Each package loads once. Packages export
  functions, families, and `fixed`/`hot` bindings; names starting with `_` are
  private. Circular imports are rejected.

See [Packages](../api/language.md#packages).

## Standard library in Marslang

`std.math` (42 functions, six constants) is written entirely in Marslang in
`std/math.mars`, including argument checks, rounding, signs, integer `pow`, `hypot`,
and interpolation. Only irreducible primitives are native Rust packages under
`std/rs/`: `rs.core` (raise a named error, read a value's kind) and `rs.math`
(platform float functions). Only standard packages may import `rs.*`.

`build.rs` registers every `std/*.mars` file as `std.NAME` and every `std/rs/*.rs`
file as `rs.NAME`, so adding a standard package needs no list edits.

## Validation

`cargo test` passes 78 tests (9 unit, 69 execution, none ignored) on Windows Rust
and on WSL-native Rust. Execution tests run programs in the interpreter and assert
their output or runtime errors.

## Remaining work

A per-user package directory (such as `C:\Users\<user>\marslang_pkgs`) and an
installer are planned. Wildcard imports, decorators, deque, further standard
packages, `match`, `run/handle/then`, async, and self-hosting remain pending.
