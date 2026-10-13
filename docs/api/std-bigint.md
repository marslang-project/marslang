# std.bigint

Whole numbers of any size. `bigint` is the fourth numeric kind, beside `int`,
`longint`, and `float`: a whole-number literal too large for a `longint` is
one, `bigint(x)` makes one, and the arithmetic operators work on it exactly, so
`factorial(100)`, `2 ** 200`, and sums of money in cents beyond 2^63 come out
right to the last digit. No import is needed for any of that.

```mars
func m{
    cents = 123456789012345678901234567890;   // too large for a longint: a bigint
    out(cents * 100 + 99);                    // 12345678901234567890123456789099
    out(bigint(2) ** 100);                    // 1267650600228229401496703205376
    out(bigint(7) / 2, bigint(8) / 2);        // 3.5 4
    out(bigint(10) == 10, bigint(10) < 10.5); // true true
}
```

The package `std.bigint` adds what the operators do not: floor division,
gcd and lcm, square roots, factorials, and bases other than ten.

```mars
takepkg std.bigint;

func m{
    out(bigint.factorial(30));                            // 265252859812191058636308480000000
    out(bigint.divide(-7, 2), bigint.remainder(-7, 2));   // -4 1
    out(bigint.digits(255, 2), bigint.parse("ff", 16));   // 11111111 255
}
```

## The bigint kind

| Written | Means |
| --- | --- |
| `123456789012345678901234567890` | A bigint: a whole-number literal beyond `longint` |
| `bigint(x)` | `x` as a bigint: from an int, longint, bigint, whole float, or string of decimal digits |
| `x (bigint) = 5;` | A bigint binding; an int or longint assigned to it is converted |
| `func f(bigint n)` | A bigint parameter; an int or longint passed to it is converted |
| `[int,longint,bigint] n` | Any whole number, keeping its kind |
| `array[bigint]` | An array of bigints; ints and longints added to it are converted |

`types.kind(x)` is `"bigint"`, and `types.is_number(x)` is `true`. A bigint
stays a bigint even when it is small: `bigint(2) - 1` is the bigint `1`, not an
int. Convert with `int(...)` where an int is needed, such as an array index.

## Operators

| Operation | Result |
| --- | --- |
| `+`, `-`, `*`, unary `-` | A bigint, exact |
| A bigint with an int or longint, on either side | A bigint, exact: `bigint(1) + 1` is the bigint `2` |
| A bigint with a float | `TypeError`: convert one side first, as with a longint and a float |
| `a / b` | The exact quotient, a bigint, when `b` divides `a`; otherwise the nearest float, as with ints |
| `a % b` | The remainder, with the sign of `a`, as with ints: `bigint(-7) % 2` is `-1` |
| `a ** b` | A bigint for `b` of 0 or more; for a negative `b`, `1 / a ** -b`, as below |
| `==`, `!=`, `<`, `<=`, `>`, `>=` | Exact, against a number of any kind |

Division and remainder by zero raise `RangeError: division by zero`, as they do
for ints.

**Nothing becomes a bigint by itself.** Int and longint arithmetic still raises
on overflow, as before; a calculation is in bigints only once a bigint is in
it, so a mistake in ordinary code is not hidden by a slower, larger number:

```mars
func m{
    run{ out(2147483647 + 1); } handle(RangeError e){ out(e.message); }   // int overflow
    out(bigint(2147483647) + 1);                                          // 2147483648
}
```

**Floats.** A float holds 53 bits, so mixing one with a bigint would round
silently: `bigint(2) ** 60 + 0.5` is a `TypeError`, as `longint(5) + 0.5` is.
Write `float(b) + 0.5` or `b + bigint(f)`, whichever you mean. Comparisons are
exact all the same, never made by rounding the bigint to a float:

```mars
func m{
    b = bigint(2) ** 53 + 1;
    out(b == 9007199254740992.0);   // false
    out(b > 9007199254740992.0);    // true
    out(float(b));                  // 9007199254740992
}
```

**Division** follows the int rule: `bigint(8) / 2` is the bigint `4`, and
`bigint(7) / 2` is the float `3.5`, the nearest float to the exact quotient even
when both numbers are far beyond a float's range. A quotient that is not whole
and is beyond the largest float, about 1.8e308, raises `RangeError` rather than
giving infinity; a tiny one becomes `0.0`, as for floats. For whole-number
division, use [`bigint.divide`](#divide-and-remainder).

**Powers with a negative exponent** follow the int rule too: `x ** -n` is
`1 / x ** n`, a float, so `bigint(2) ** -2` is `0.25`. `1` and `-1` to a
negative power stay bigints, and `bigint(0) ** -1` raises division by zero.
`bigint.pow` never gives a float.

**Equality and keys.** Numbers are equal by exact value across all four kinds,
so `bigint(1) == 1`, `bigint(1) == 1.0`, and `bigint(1) == longint(1)`, and equal
numbers are the same map key and the same set element:

```mars
func m{
    names = map();
    names.set(1, "one");
    out(names.get(bigint(1)));            // one
    out(set(1, bigint(1), 1.0).len());    // 1
}
```

## Conversions

| Call | Gives |
| --- | --- |
| `bigint(x)` | `x` as a bigint; see below |
| `int(b)`, `longint(b)` | The same number as an int or longint; `RangeError` when it does not fit |
| `float(b)` | The nearest float; `RangeError` beyond about 1.8e308, rather than infinity |
| `string(b)`, `out(b)`, `f"{b}"` | Every digit, never an exponent: `123456789012345678901234567890` |

`bigint(x)` takes:

- an `int`, `longint`, or `bigint`, unchanged in value;
- a whole, finite `float`, exactly: `bigint(1e20)` is `100000000000000000000`. A
  float with a fraction, infinity, or NaN raises `RangeError`, since there is no
  one integer it stands for; round it first, with `math.round`.
- a string of decimal digits, with `-` in front of a negative number. Anything
  else, even a space, `+`, `_`, or `0x`, raises `SyntaxError` quoting the
  text. `bigint.parse(text, base)` reads other bases.

Annotations only convert upward: an int or longint becomes a bigint where a
`bigint` is declared, as an int becomes a float where a `float` is, and so
does a whole float up to 2^53, as it would become a longint. A bigint never
narrows by itself: passing one to an `int`, `longint`, or `float` parameter
raises a `TypeError` that names the conversion to write. The
[std.math](std-math.md) functions take `[int,longint,float]`, so they refuse a
bigint the same way; this package has `pow`, `sqrt`, `gcd`, `lcm`, `abs`, and
`factorial` for bigints.

[std.json](std-json.md) writes a bigint as a JSON number with every digit, and
reads a whole number beyond `longint` as a bigint, so
`123456789012345678901234567890` makes the round trip unchanged.

## Limits

A bigint holds at most 3,321,929 bits, which is every whole number of up to a
million decimal digits. A result beyond that raises
`RangeError: bigint too large`, and so does a literal, a string, or a JSON
number that size. `**` and `*` work out the size first, so
`bigint(2) ** 100000000000` raises at once instead of running out of memory.
`bigint.factorial(n)` takes `n` up to 205,022, whose factorial has a million
digits.

The operators stay quick up to the limit. In a release build, multiplying two
numbers of half a million digits takes about 0.05 seconds and dividing one by
the other 0.1; writing out a million digits takes 0.35, reading them back 0.15,
and `bigint.sqrt` of them 1.4. Numbers of a few thousand digits take
microseconds: about 1 to add, 20 to multiply, and 60 to write out.
`bigint.gcd` is Euclid's algorithm in Marslang, one loop step per remainder:
about half a second at worst for two numbers of ten thousand digits.

## Functions

| Function | Result |
| --- | --- |
| `of(value)` | The built-in `bigint(value)`, for files where `bigint` names this package |
| `pow(base, exponent)` | `base ** exponent`; `exponent` must be 0 or more |
| `gcd(a, b)` | The greatest common divisor, never negative; `gcd(0, 0)` is `0` |
| `lcm(a, b)` | The least common multiple, never negative; `0` when either is `0` |
| `sqrt(n)` | The integer square root: the largest number whose square is at most `n` |
| `factorial(n)` | `n!`, for `n` from 0 to 205,022 |
| `divide(a, b)` | `a / b` rounded down, toward negative infinity |
| `remainder(a, b)` | `a - b * divide(a, b)`, which has the sign of `b` |
| `parse(text, base)` | The number `text` is written as in a base from 2 to 36 |
| `digits(n, base)` | `n` written in a base from 2 to 36, as a string |
| `abs(n)` | The absolute value |
| `sign(n)` | `-1`, `0`, or `1`, by the sign of `n` |

Numbers passed in may be ints, longints, or bigints, and every number that
comes back is a bigint, `sign` included. A negative `n` to `sqrt` or
`factorial`, a negative exponent to `pow`, a zero `b` to `divide` or
`remainder`, and a base outside 2 to 36 raise `RangeError`.

### divide and remainder

`divide` rounds down and `remainder` takes the sign of the divisor, as floor
division does in most mathematics. The operators keep the int rules instead:
`/` gives a float when the division is not exact, and `%` takes the sign of the
dividend.

| | `-7` and `2` | `7` and `-2` |
| --- | --- | --- |
| `a / b` | `-3.5` | `-3.5` |
| `a % b` | `-1` | `1` |
| `bigint.divide(a, b)` | `-4` | `-4` |
| `bigint.remainder(a, b)` | `1` | `-1` |

`a == b * bigint.divide(a, b) + bigint.remainder(a, b)` always holds.

### parse and digits

```mars
takepkg std.bigint;

func m{
    out(bigint.parse("-101", 2), bigint.parse("FF", 16), bigint.parse("zz", 36));   // -5 255 1295
    out(bigint.digits(bigint.of(2) ** 64, 16));                                     // 10000000000000000
}
```

The letters `a` to `z` are the digits from 10 up; `parse` takes them in either
case and `digits` writes them in lower case. `parse` takes a `-` in front of a
negative number and nothing else: a space, a `+`, or a prefix such as `0x`
raises `SyntaxError`.

### Importing it, and the name bigint

`takepkg std.bigint;` names the package `bigint` in your file, as
`takepkg std.Error;` names its package `Error`. There, `bigint(x)` would call the
package, so the built-in is `bigint.of(x)`; calling the package raises a
`TypeError` that says so. To keep the bare name, import the package under
another one:

```mars
takepkg std.bigint = big;

func m{
    out(big.sqrt(bigint(10) ** 30));   // 1000000000000000
}
```

Type names are not affected: `(bigint)` and `bigint n` mean the kind in every
file.

## How it works

A bigint is a `Value::Big` holding a [num-bigint](https://docs.rs/num-bigint)
number behind a reference count, so values are no larger than before and
copying one is cheap. It holds no other values, so the cycle collector never
tracks it, as it never tracks a string.

The interpreter keeps its int, longint, and float paths first, unchanged; only
when an operand is a bigint does arithmetic go to
[src/bigint.rs](../../src/bigint.rs), so programs that use no bigints run as
fast as before. Equality against a float compares the bigint with the float's
whole part exactly, and the fraction only settles a tie. A bigint within
`longint` range has the same map key as the int or longint it equals, and a
larger one the key of the float that holds it exactly, if any, so equal numbers
always share a key. `/` rounds its float quotient from the exact quotient, ties
to even, as division of floats does.

Literals are read when the program is compiled. Long decimal numbers are read
by halves, then joined, so a million digits take a fraction of a second rather
than the time growing with the square of their length; this matters for JSON
from elsewhere.

The package is Marslang ([std/bigint.mars](../../std/bigint.mars)) on top of
the operators: `gcd` is Euclid's algorithm, and `factorial` multiplies halves
of its range together so that most multiplications are of numbers of similar
size. Square roots and bases are native
([std/rs/bigint.rs](../../std/rs/bigint.rs)), where num-bigint does them on
whole machine words.
