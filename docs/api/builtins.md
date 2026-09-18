# Built-ins and errors

## Input and output

| Call | Behavior |
| --- | --- |
| `out(value,...)` | Print values separated by spaces, followed by a newline |
| `slout(value,...)` | Print values consecutively without a newline |
| `in()` | Read standard input to the end as a string |
| `inln()` | Read successive buffered lines; currently returns `""` after exhaustion |

Input is synchronous in the Node backend. `inln()` buffers standard input; mixing
`in()` and `inln()` is not a supported streaming input model. Container display
format is not a stable serialization format.

## Conversions and copying

| Call | Behavior |
| --- | --- |
| `int(value)` | Convert to a number, then require an integer within signed 32-bit bounds |
| `longint(value)` | Require an exact signed 64-bit integer; decimal integer strings are accepted |
| `float(value)` | Convert using the bootstrap backend's numeric conversion |
| `string(value)` | Convert using the bootstrap backend's string conversion |
| `value.copy()` | Deep-copy containers/objects; immutable scalar values are returned as values |

Prefer decimal strings for converting very large numbers to `longint`, so they
cannot be rounded before conversion. Full conversion semantics, including all
invalid-string cases, are not yet a portable language contract.

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
Errors currently terminate execution with a diagnostic and nonzero exit status.
Marslang `run/handle/then` and user-defined error raising are not implemented yet;
these names are not currently callable Marslang error constructors.

Undefined variables, invalid syntax, and fixed-binding reassignment can fail
during compilation. Full source-location diagnostics remain pending.
