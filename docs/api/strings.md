# Strings and slicing

Both single and double quotes create strings. Boolean literals are `true` and
`false` (lowercase). Strings are immutable values: these methods return results
without changing the original binding.

## Characters

A character is one Unicode extended grapheme cluster: a visible character can
contain multiple Unicode code points. Chinese `中`, Japanese `あ`, Arabic `عَ`,
`é`, `🇨🇳`, `👍🏽`, and `👨‍👩‍👧‍👦` each count as one.

Length, iteration, reversal, and slicing all use this unit. They do not normalize
the text. Segmentation uses the Rust `unicode-segmentation` crate (Unicode
extended grapheme cluster rules); its Unicode version follows the crate version.

## Methods

| Call | Result |
| --- | --- |
| `text.len()` | Number of characters |
| `text.reverse()` | New string with characters in reverse order |
| `text.slice(start,end)` | New string selecting positions `[start,end)` |
| `text.lenslice(start,length)` | New string selecting `length` characters from `start` |
| `text.copy()` | The same immutable string value |

`len()` and `reverse()` take no arguments. Use `text.lenslice(index,1)` to extract
one character; string bracket indexing is not implemented.

## Counting from the end

Both slicing methods accept an optional third boolean argument. It defaults to
`false`. Write `reverse=true` or pass `true` positionally. A boolean expression is
also accepted. Named `reverse=` must follow exactly two positional arguments and
must be last. No other named arguments are supported.

With `reverse=true`, position zero is the last character. Selection proceeds
toward the beginning, but the result keeps the original text order.

| Expression | Result |
| --- | --- |
| `"ABCDE".lenslice(0,3)` | `"ABC"` |
| `"ABCDE".lenslice(0,3,reverse=true)` | `"CDE"` |
| `"ABCDE".lenslice(1,3,reverse=true)` | `"BCD"` |
| `"ABCDE".slice(1,4,reverse=true)` | `"BCD"` |
| `"ABCDE".lenslice(0,3,reverse=true).reverse()` | `"EDC"` |

For `slice`, the end remains exclusive in the chosen direction. For a string of
length `N`, reverse `slice(start,end)` selects the forward interval
`[N-end,N-start)`. Reverse `lenslice(start,length)` selects
`[N-start-length,N-start)`.

## Bounds and errors

- Positions and lengths must be integers. Fractional values, strings, and booleans
  raise `TypeError`; `longint` values are accepted when within range.
- Negative positions/lengths, starts beyond the string length, reversed intervals
  (`end < start`), and ranges exceeding available characters raise `OutOfBoundsError`.
- A zero-length slice returns `""`. Its start may equal the string length, in either
  direction. The empty string accepts `lenslice(0,0)` and `slice(0,0)`.
- Two or three arguments are required. The optional reverse value must be a boolean;
  invalid arity or flag type raises `TypeError`.

Slices never silently truncate. For example, `"ABC".lenslice(0,4)` raises
`OutOfBoundsError`, and `"ABC".lenslice(3,0,reverse=true)` returns `""`.

## Iteration

```mars
func m{
    for (ch, "中é👨‍👩‍👧‍👦"){
        out(ch);
        out(ch.len()); // Each prints 1.
    }
}
```

Only the string APIs listed here are supported language contracts. Calling any
other method on a string raises `TypeError`.
