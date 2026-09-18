# rs-0.5.0 — Initial math package design

This release introduces the first Marslang-written standard-library package and
runtime numeric type tracking. **The math package is an early, provisional design:
more functionality will be added, and its API may evolve.**

## std.math

`takepkg std.math;` provides `math.min`, `math.max`, `math.abs`, and `math.clamp`.
Explicit aliases and storing functions in variables are supported. The algorithms
are written in `std/math.mars`, compiled by Rust, and bundled into the generated
JavaScript with the shared runtime.

```mars
takepkg std.math;
func m{
    out(math.clamp(12,0,10)); // 10
}
```

Arguments must have matching numeric types (`int`, `longint`, or finite `float`).
Clamp includes both bounds and rejects reversed bounds. Integer overflow,
incorrect argument counts, and invalid numeric arguments raise runtime errors.
See the [math API](../api/std-math.md) for details and current limits.

## Dynamic typing and numeric behavior

Unannotated variables can change type on reassignment. Annotated bindings retain
runtime checks, and `fixed` still prevents reassignment. Numeric kinds survive
function calls, returns, containers, family fields, and copies: `1.0` remains a
float even when its value is integral. Numeric unions prefer the existing kind.

Arithmetic now uses runtime operand kinds, including through union-typed calls.
Integer arithmetic checks overflow; float arithmetic retains float results.
Mixed longint/float arithmetic requires an explicit conversion. Scalar equality
and collection lookups retain their value-based numeric behavior. Fixed containers
cannot have their numeric kinds changed through a differently annotated alias.

Migration: integer calculations that previously bypassed overflow checks through
unannotated or union-typed values can now raise an error. Use `longint` or `float`
explicitly when those numeric semantics are required. `:=` remains excluded.

## Repository and validation

- `.mars` and `.js.inc` files are excluded from GitHub Linguist language detection.
- The public API reference documents the package and dynamic typing rules.
- `cargo test` passes 63 tests: 7 unit and 56 integration, none ignored, using
  WSL-native Rust with Windows Node. The toolchains use separate target directories.

## Remaining work

More math APIs, decimal/fraction support, full filesystem modules/exports, other
standard packages, decorators, and self-hosting remain pending. Infinity/NaN math
semantics are deferred; this initial package requires finite float inputs.
