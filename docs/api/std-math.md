# std.math

Introduced in `rs-0.5.0`. This is an initial, provisional package design, not the
complete math library. More functions and numeric facilities will be added;
APIs may be refined as the language develops.

The working tree after rs-0.5.0 expands this to **42 functions and six constants**.

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

## Constants

Added in the working tree after rs-0.5.0: `math.PI`, `math.E`, `math.TAU`,
`math.SQRT2`, `math.LN2`, and `math.LN10`. These are read-only float values, accessed
without parentheses. They use the backend's binary64 precision. `TAU` is twice PI;
`LN2` and `LN10` are natural logarithms. INF and NAN constants are not provided yet.

## Original functions

| Function | Result |
| --- | --- |
| `min(a,b)` | Smaller value |
| `max(a,b)` | Larger value |
| `abs(value)` | Absolute value |
| `clamp(value,low,high)` | Value constrained to the inclusive interval `[low,high]` |

Each function above accepts `int`, `longint`, or finite `float` values and returns the
same numeric type. Multi-argument calls require matching types. The exact argument
counts shown above are required. Wrong argument types/counts raise `TypeError`.

`math.min(1,2.0)` raises `TypeError`: `1` is an integer and `2.0` is a float.
Use `math.min(float(1),2.0)` to request the conversion. Runtime type tracking
preserves this distinction through variables, function calls, collections, copies,
and family fields, even when a float has an integral value.

`clamp` raises `RangeError` when `low > high`. Integer arguments/results must fit
their signed 32-bit or 64-bit range. `abs(-2147483648)` and the corresponding
minimum `longint` raise overflow errors. Non-finite floats, including NaN in any
argument of min/max/clamp, raise `RangeError`. Float zero ties are order-independent:
min chooses negative zero if either argument is negative zero, and max chooses
positive zero if either is positive zero. Abs returns positive zero. Clamp keeps
the input's zero sign when no bound replacement is needed.

## Shared rules for the expansion

Float-only functions reject integer arguments: use `float(value)` explicitly.
All arguments must be finite except for the three classification helpers. Invalid
domains, zero divisors, integer overflow, and non-finite results raise `RangeError`.
Non-finite arithmetic intermediates in the Marslang algorithms also raise errors.
Floating-point underflow to a finite subnormal or zero is allowed.

The signatures below specify exact argument counts. Wrong counts, wrong types,
or mixed numeric kinds raise `TypeError`. Angles for trig functions are in radians.

## Sign helpers

| Function | Input and result |
| --- | --- |
| `sign(x)` | Any finite numeric kind; returns int `-1`, `0`, or `1`; both zero signs return `0` |
| `signbit(x)` | Any finite numeric kind; returns boolean, true for negative values including float `-0.0` |
| `copysign(x,y)` | Matching numeric kinds; returns magnitude of x with y's sign, preserving the kind |

Integers have no negative zero. `copysign(INT_MIN,negative)` is valid, while
requesting a positive result that cannot fit raises overflow. Float copysign
uses y's zero sign and retains a zero magnitude when x is zero.

## Interpolation and angles

All arguments and results in this section are floats.

| Function | Definition |
| --- | --- |
| `lerp(a,b,t)` | `a + (b-a)*t`; allows extrapolation outside t in `[0,1]` |
| `inverse_lerp(a,b,value)` | `(value-a)/(b-a)`; a equal to b raises an error |
| `remap(value,in_low,in_high,out_low,out_high)` | Interpolate between output bounds using the input fraction; equal input bounds raise an error |
| `step(edge,x)` | `0.0` when x is below edge, otherwise `1.0` |
| `radians(degrees)` / `degrees(radians)` | Convert angles |

Inverse interpolation and remapping allow reversed intervals and extrapolation;
they do not clamp. Their direct formulas can overflow on extreme finite inputs,
in which case they raise an error rather than silently return a non-finite result.

## Rounding

All four functions accept one float and return a float, even for integral results.

| Function | Behavior |
| --- | --- |
| `floor(x)` | Round toward negative infinity |
| `ceil(x)` | Round toward positive infinity |
| `trunc(x)` | Discard the fractional part, toward zero |
| `round(x)` | Nearest integer, with exact half ties going to the even integer |

Examples: `round(2.5)` is `2.0`, `round(3.5)` is `4.0`, and `round(-2.5)` is `-2.0`.
Negative inputs that round to zero keep negative zero; inspect it using `signbit`.

## Powers and roots

| Function | Types and behavior |
| --- | --- |
| `sqrt(x)` | Float to float; negative x raises an error |
| `cbrt(x)` | Float to float; negative inputs are supported |
| `hypot(a,b)` | Two floats to float; scaled to avoid overflow and underflow, written in Marslang |
| `pow(base,exponent)` | Matching numeric kinds; preserves the kind |

Integer pow requires a nonnegative exponent and computes exact integer powers
with checked, bounded intermediates. Large exponents cannot create arbitrarily
large temporary integers. Float pow permits negative exponents and rejects
undefined real-valued results or overflow. Zero to the zero power returns one of
the input kind. A negative base with a fractional float exponent raises an error.

## Exponentials and logarithms

All arguments/results are floats. These call the native `rs.math` primitives.

| Function | Behavior/domain |
| --- | --- |
| `exp(x)` / `exp2(x)` | e to the x / 2 to the x |
| `ln(x)` / `log2(x)` / `log10(x)` | Natural / base-2 / base-10 logarithm; x must be positive |
| `log(x,base)` | Arbitrary-base logarithm; x and base positive, base unequal to one |
| `expm1(x)` | Accurate exp(x) minus one for small x |
| `log1p(x)` | Accurate ln(1+x) for small x; x must exceed -1 |

## Trigonometry

All inputs/results are floats; angles use radians.

| Functions | Behavior/domain |
| --- | --- |
| `sin(x)`, `cos(x)`, `tan(x)` | Standard trigonometric functions |
| `asin(x)`, `acos(x)` | Inverse sine/cosine; x in `[-1,1]` |
| `atan(x)` | Inverse tangent |
| `atan2(y,x)` | Quadrant-aware angle; note y comes first; signed-zero behavior follows the platform math library |
| `sinh(x)`, `cosh(x)`, `tanh(x)` | Hyperbolic functions |

## Infinity, NaN, and classification

There is **no `inf` keyword**. Construct float values using `float("inf")`,
`float("-inf")`, or `float("nan")`. Infinity spellings are case-insensitive,
accept surrounding whitespace, and support `inf`/`infinity` with an optional sign.
There are no `math.INF` or `math.NAN` constants.

| Function | Result |
| --- | --- |
| `is_nan(x)` | Whether float x is NaN |
| `is_inf(x)` | Whether float x is positive or negative infinity |
| `is_finite(x)` | Whether float x is neither infinity nor NaN |

These three functions require float inputs and return booleans. They are the
exception to the package's finite-input rule. Constructing infinity is supported;
passing it to arithmetic functions such as `math.sin` still raises an error.

## Integers

These take `int` or `longint` arguments of one matching kind (floats raise
`TypeError`) and return that kind. Overflow raises the kind's overflow error.

| Function | Result |
| --- | --- |
| `gcd(a,b)` | Greatest common divisor, never negative; `gcd(0,0)` is 0 |
| `lcm(a,b)` | Least common multiple, never negative; 0 when either is 0 |
| `is_even(n)` / `is_odd(n)` | Boolean |
| `div_floor(a,b)` / `div_ceil(a,b)` | Quotient rounded toward negative/positive infinity; zero divisor raises `RangeError` |
| `factorial(n)` | `n!`; negative `n` raises `RangeError` |
| `perm(n,k)` / `comb(n,k)` | Ordered/unordered selections of `k` from `n`; 0 when `k > n`; negative arguments raise `RangeError` |

`comb` divides out common factors at each step, so it succeeds whenever the
result fits: `comb(longint(66), longint(33))` is `7219428434016265740`.

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

The package source is built into the `marslang` executable, so running a program
does not require locating a separate library file.

## Implementation

The whole public API is Marslang: [std/math.mars](../../std/math.mars). That file checks
arity, matching numeric kinds, float-only inputs, finite inputs, and finite results,
and implements min/max/abs/clamp, signs, `copysign`, rounding (truncation via `% 1.0`,
floor, ceil, half-to-even), checked integer `pow`, `hypot`, `log`, and interpolation.

It imports two native packages written in Rust: `rs.core`
([std/rs/core.rs](../../std/rs/core.rs)) to raise named errors and read a value's kind,
and `rs.math` ([std/rs/math.rs](../../std/rs/math.rs)) for the platform's float primitives:
`sqrt`, `cbrt`, `exp`, `expm1`, `ln`, `log1p`, `log2`, `log10`, trigonometric and
hyperbolic functions, `atan2`, float `pow`, and the float sign bit.

Approximate equality, adjacent-float operations, inverse hyperbolic functions,
decimal/fraction, and the optional convenience functions from the expansion proposal
remain future work. Package boundaries for statistics,
random generation, complex numbers, and linear algebra remain unchanged.
