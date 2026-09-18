# rs-0.3.0

Rust bootstrap compiler release, 2026-09-18. Backend: JavaScript running on Node.

## Core language and runtime

- Structured expressions and lexical scope resolution: bare assignment updates
  the nearest existing binding; annotations and `cold` explicitly declare locals.
- Runtime type checks, checked signed 32/64-bit integers, noncoercing equality,
  identity-based container equality, and boolean short-circuiting.
- Fixed bindings, recursive container immutability through aliases, mutable deep
  `.copy()` with cycle/shared-reference preservation, and hot constant substitution.
- `while`, iterable and three-part `for`, `break`, and `continue`; snapshot iteration.
- Working array/set/pair APIs and insertion-ordered `map()`/`dict()` with null for
  missing keys, inspection methods, and typed collection mutation checks.

## Unicode strings and slicing

String length, iteration, reversal, and slicing count extended grapheme clusters.
Combining marks and joined emoji stay together. Node must provide `Intl.Segmenter`.
String `.reverse()` returns a new string without changing the original.

String and array `.slice(start,end)` and `.lenslice(start,length)` accept an
optional third boolean argument, including the slicing-only `reverse=true` form.
Reverse slicing counts from the end and preserves the original order:

```mars
func m{
    out("ABCDE".lenslice(0,3,reverse=true)); // CDE
}
```

Invalid bounds raise `OutOfBoundsError`. Zero-length slices are valid at the end.
Incorrect argument types or counts raise `TypeError`.

## Fixes and migration notes

The bundled example now executes successfully. Regressions addressed include
repeated assignment, comparisons mistaken for declarations, comment markers in
strings, compact blocks, compound `me` expressions, and shadowed built-ins.

Explicit redeclarations in the same scope and undefined names are rejected.
Programs relying on accidental JavaScript coercion or out-of-range array slicing
must be updated. Array slices now use strict bounds rather than silently clipping
or accepting negative indices. Array reversal still mutates the array.

Comma-separated function parameters from rs-0.2.0 remain required.

## Documentation and validation

The [API reference](../api/README.md) covers syntax, strings, collections,
built-ins, and errors, with a runnable example. Discussion documents are removed
from the tracked tree and remain local; `docs/api/` and `docs/releases/` are exempt
from the documentation ignore rule.

Validation: `cargo test` passes 55 tests (7 unit and 48 integration), none ignored.
Execution tests run generated JavaScript under Node and cover the API example.

## Remaining limitations

Self-hosting, a unified lexer/parser with source locations, complete modules and
exports, standard-library packages, decorators, deque, async, `match`, and
`run/handle/then` remain pending. General named arguments are deferred. Unicode
segmentation follows the host's Unicode/ICU version. The compiler still targets
Node rather than a native runtime.
