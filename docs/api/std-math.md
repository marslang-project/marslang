# std.math

Introduced in `rs-0.5.0`. This is an initial, provisional package design, not the
complete math library. More functions and numeric facilities will be added;
APIs may be refined as the language develops.

```mars
takepkg std.math;

func m{
    out(math.min(3,7));
    out(math.max(3,7));
    out(math.abs(-5));
    out(math.clamp(12,0,10));
    out(math.min(1.0,2.0));
}
```

Output: `3`, `7`, `5`, `10`, `1`, each on its own line.

## Functions

| Function | Result |
| --- | --- |
| `min(a,b)` | Smaller value |
| `max(a,b)` | Larger value |
| `abs(value)` | Absolute value |
| `clamp(value,low,high)` | Value constrained to the inclusive interval `[low,high]` |

Each function accepts `int`, `longint`, or finite `float` values and returns the
same numeric type. Multi-argument calls require matching types. The exact argument
counts shown above are required. Wrong argument types/counts raise `TypeError`.

`math.min(1,2.0)` raises `TypeError`: `1` is an integer and `2.0` is a float.
Use `math.min(float(1),2.0)` to request the conversion. Runtime type tracking
preserves this distinction through variables, function calls, collections, copies,
and family fields, even when a float has an integral value.

`clamp` raises `RangeError` when `low > high`. Integer arguments/results must fit
their signed 32-bit or 64-bit range. `abs(-2147483648)` and the corresponding
minimum `longint` raise overflow errors. Non-finite floats raise `RangeError`;
infinity/NaN behavior for this package is deferred.

## Importing and passing functions

Use `takepkg std.math = calc;` to call `calc.min(...)`. Without an alias the name is
`math`. Repeated imports of the same package under the same alias are deduplicated.
Aliases refer to one immutable module namespace. Functions can be stored and called:

```mars
takepkg std.math;
func m{
    smaller = math.min;
    out(smaller(4,9));
}
```

The compiler bundles the package into generated JavaScript, so running the output
does not require locating a separate library file. General filesystem modules,
wildcard imports, and other standard packages remain pending.

## Implementation

The algorithms live in [std/math.mars](../../std/math.mars). Rust compiles that
source using the same parser/resolver/emitter as user code. The Node runtime checks
the numeric contract at the package boundary. The package shares the program's
runtime; it does not load a second copy of the collection or numeric machinery.

Decimal, fraction, and additional mathematics APIs remain future work.
