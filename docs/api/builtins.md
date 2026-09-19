# Built-ins and errors

## Input and output

| Call | Behavior |
| --- | --- |
| `out(value,...)` | Print values separated by spaces, followed by a newline |
| `slout(value,...)` | Print values consecutively without a newline |
| `in()` | Read standard input to the end as a string |
| `inln()` | Read successive buffered lines; currently returns `""` after exhaustion |

Input is synchronous. `inln()` buffers standard input; mixing
`in()` and `inln()` is not a supported streaming input model. Container display
format is not a stable serialization format.

## Conversions and copying

| Call | Behavior |
| --- | --- |
| `int(value)` | Convert to a number, then require an integer within signed 32-bit bounds |
| `longint(value)` | Require an exact signed 64-bit integer; decimal integer strings are accepted |
| `float(value)` | Convert to float, including `"inf"`, `"-inf"`, and `"nan"` strings |
| `string(value)` | Convert to text: numbers as `out` prints them (but `-0.0` gives `"0"`), containers in display form |
| `value.copy()` | Deep-copy containers/objects; immutable scalar values are returned as values |

Prefer decimal strings for converting very large numbers to `longint`, so they
cannot be rounded before conversion. Full conversion semantics, including all
invalid-string cases, are not yet a portable language contract.

There is no `inf` keyword. Use `float("inf")` or `float("-inf")` to construct
infinity. Infinity spellings accept either case, surrounding whitespace, and
`inf`/`infinity` with an optional sign. [std.math](std-math.md) provides float
classification helpers; its arithmetic functions require finite inputs/results.

Constructors `arr`, `set`, `pair`, `map`, and `dict` are documented in
[Collections](collections.md).

## Runtime errors

| Error | Current examples |
| --- | --- |
| `OutOfBoundsError` | Negative slice positions/lengths, invalid ranges, oversized slices |
| `TypeError` | Slice argument type/arity errors; mismatched typed values |
| `RangeError` | Integer overflow, division by zero, invalid array element indices |
| `Error` | Mutation through a fixed alias and other runtime contract violations |

`OutOfBoundsError` is the runtime error name, not just text inside a generic error.
Errors currently terminate execution with `error: <Kind>: <message>` on standard error
and exit status 1. `SyntaxError` is raised for invalid `longint(...)` strings.
Calls nested deeper than 10,000 levels raise `RangeError: maximum call depth exceeded`.
Calling a function, method, or built-in with the wrong number of arguments raises `TypeError`.
Marslang `run/handle/then` and user-defined error raising are not implemented yet;
these names are not currently callable Marslang error constructors.

Undefined variables, invalid syntax, and fixed-binding reassignment can fail
during compilation. Full source-location diagnostics remain pending.
