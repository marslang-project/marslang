# std.format

Numbers and values made ready to show: decimal places, thousands, percentages,
columns, tables, ordinals, and other bases. Each function returns a string, so
they fit inside [f-strings](strings.md#f-strings).

```mars
takepkg std.format;

func m{
    price = 1234.5;
    out(f"Total: {format.number(price, 2)}");          // Total: 1,234.50
    out(f"Tax: {format.percent(0.075, 1)}");           // Tax: 7.5%
    out(format.table(arr(
        arr("Name", "City", "Age"),
        arr("Ada", "London", 36),
        arr("Grace", "New York", 85)
    )));
}
```

prints

```text
Total: 1,234.50
Tax: 7.5%
Name   City      Age
Ada    London     36
Grace  New York   85
```

## Numbers

| Function | Result |
| --- | --- |
| `fixed(x, places)` | Rounded to `places` decimals, every place written: `fixed(3.14159, 2)` is `"3.14"` |
| `number(x, places)` | As `fixed`, with commas between thousands: `"1,234,567.89"` |
| `thousands(x)` | The number as `string(x)` writes it, with commas: `"1,234,567"` |
| `percent(x, places)` | A fraction as a percentage: `percent(0.256, 1)` is `"25.6%"` |
| `scientific(x, places)` | Scientific notation: `scientific(123456, 2)` is `"1.23e+5"` |

Rounding works from the number's exact value. A float stores 2.675 as
2.67499..., so `fixed(2.675, 2)` is `"2.67"`, as in Python and JavaScript; a
value exactly halfway rounds to the even digit, so `fixed(2.5, 0)` is `"2"` and
`fixed(3.5, 0)` is `"4"`. Ints and longints are exact at any size. A negative
number that rounds to zero shows no sign: `fixed(-0.001, 2)` is `"0.00"`.
`places` is from 0 to 100.

## Columns and tables

| Function | Result |
| --- | --- |
| `left(value, columns)` | The value as text, with spaces after it to fill `columns` |
| `right(value, columns)` | With spaces before it, so numbers line up on the right |
| `center(value, columns)` | Centred; an odd space goes after it |
| `table(rows)` | An array of rows, each an array of values, as lines of aligned columns |
| `width(value)` | How many columns the value takes in a terminal, as text |

Widths are terminal columns, not characters: Chinese, Japanese, and Korean
characters and most emoji take two, and combining accents none, so
`format.width("你好")` is `4` and tables line up whatever they hold. Text
longer than the width is left whole.

`table` makes each column as wide as its widest cell, with two spaces between
columns. Numbers line up on the right and everything else on the left; a row
may have fewer cells than the others, and no line ends in spaces. The result
has no newline at the end, so `out` prints it as it is.

## Whole numbers

| Function | Result |
| --- | --- |
| `ordinal(n)` | With its English ending: `"1st"`, `"22nd"`, `"13th"` |
| `hex(n)` | In hexadecimal, lower case, without `0x`: `hex(255)` is `"ff"` |
| `binary(n)` | In binary: `binary(10)` is `"1010"` |
| `base(n, radix)` | In any base from 2 to 36, with digits 0-9 then a-z: `base(35, 36)` is `"z"` |

Negative numbers start with `-`, and every `longint` works, the smallest
included. Write `"0x" + format.hex(n)` for a prefix.

## Errors

A number of places outside 0 to 100 and a base outside 2 to 36 raise
`RangeError`; a value of the wrong kind, such as a string to `fixed`, raises
`TypeError`.

## How it works

Rounding to decimal places is native ([std/rs/format.rs](../../std/rs/format.rs)),
because it needs a float's exact value, and so is `width`, from the
`unicode-width` crate. Grouping thousands, padding, tables, ordinals, and bases
are Marslang, in [std/format.mars](../../std/format.mars).
