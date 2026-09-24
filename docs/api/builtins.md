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

Pending output is flushed before `in()` and `inln()` read, so a prompt is visible
while the program waits. When standard output is a terminal, each `out` and `slout`
is shown immediately; when it is redirected, output is buffered until input is read
or the program ends.

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

`date`, `datetime`, and `duration` build the calendar kinds; see
[dates and times](dates.md).

## Characters and code points

| Call | Behavior |
| --- | --- |
| `ord(text)` | The Unicode code point of a one-character string, as an `int` |
| `chr(number)` | The one-character string for a code point |

```mars
out(ord("a"), ord("中"), ord("😀"));   // 97 20013 128512
out(chr(97), chr(ord("A") + 2));      // a C
```

A code point is the number Unicode gives a character, from `0` to `1114111`
(`0x10FFFF`). Most characters are one code point, but some that look like one
character are several: `é` can be written as `e` followed by a combining accent,
and a family emoji joins several people. Marslang's strings count those as one
character, so `ord` of one raises `RangeError` and lists the code points it is
made of:

```text
RangeError: ord needs one code point, but "é" is 2 (U+0065 U+0301); take ord of each part
```

`chr` raises `RangeError` for a number outside that range and for `55296` to
`57343` (`U+D800` to `U+DFFF`), the surrogate halves, which UTF-16 uses in pairs
and which are not characters on their own. It accepts an `int`, a `longint`, or
a whole-number `float`.

## Runtime errors

Errors are families. `Error` is the base family; the built-in kinds below inherit
from it. See [error handling](language.md#error-handling) for `err`, `lasterr`, and
`run{} handle(...){} then{}`, and [std.Error](std-error.md) for the same families
as `Error.TypeError`, `Error.RangeError`, and so on.

| Error | Current examples |
| --- | --- |
| `OutOfBoundsError` | Negative slice positions/lengths, invalid ranges, oversized slices |
| `TypeError` | Slice argument type/arity errors; mismatched typed values |
| `RangeError` | Integer overflow, division by zero, invalid array element indices |
| `Error` | Mutation through a fixed alias and other runtime contract violations |

`SyntaxError` is raised for invalid `longint(...)` strings. An error that no
`handle` block catches ends the program with `error: <Family>: <message>` on standard
error and exit status 1.
Calls nested deeper than 10,000 levels raise `RangeError: maximum call depth exceeded`.
Calling a function, method, or built-in with the wrong number of arguments raises `TypeError`.

Undefined variables, invalid syntax, and fixed-binding reassignment fail during
compilation, before anything runs. Syntax errors and bad imports report the
source line they are on:

```text
error: line 9: expected expression, got End
error: line 2: package 'shapes.circle' not found: looked for ...
```

Name resolution has no positions yet, so `error: unknown name 'nope'` says what
is missing but not where. Editors can still place it: the
[VS Code extension](https://github.com/marslang-project/vscode-marslang) marks
the first use of the name it quotes.
