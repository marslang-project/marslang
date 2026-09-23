# std.stats

Descriptive statistics over an array of numbers: what is typical, how spread
out the numbers are, and whether two sets of numbers move together.

```mars
takepkg std.stats;

func m{
    marks = arr(2, 4, 4, 4, 5, 5, 7, 9);
    out(stats.mean(marks), stats.median(marks), stats.mode(marks));   // 5 4.5 4
    out(stats.stdev(marks));                                          // 2.138089935299395
    out(stats.quantile(marks, 0.25));                                 // 4
}
```

Every result is a float, because an average is rarely a whole number. Floats
print without a trailing `.0`, so `5` above is the float `5.0`. The numbers may
be `int`, `longint`, or `float` in any mixture; anything else raises
`TypeError`, and too few numbers raises `RangeError`.

## What is typical

| Function | Result |
| --- | --- |
| `mean(values)` | The average |
| `median(values)` | The middle number once sorted; the average of the middle two for an even count |
| `mode(values)` | The most common value, keeping its own kind; the first of a tie |
| `quantile(values, q)` | The value at fraction `q`, from `0.0` to `1.0` |
| `sum(values)` | The total |

`quantile(values, 0.5)` is the median and `quantile(values, 0.25)` the first
quartile. Between two neighbours the result is interpolated, the rule NumPy's
`percentile` and R call type 7, so `std.stats` agrees with both.

`mode` works on any values, not only numbers: `stats.mode(arr("a", "b", "a"))`
is `"a"`.

## How spread out

| Function | Divides by | Use for |
| --- | --- | --- |
| `variance(values)` | `len - 1` | A sample of something larger |
| `pvariance(values)` | `len` | A whole population |
| `stdev(values)` | — | The square root of `variance`, in the same unit as the numbers |
| `pstdev(values)` | — | The square root of `pvariance` |

Variance is the average squared distance from the mean; the standard deviation
brings that back to the unit of the numbers, so `stdev` is the one to report.
Use the sample forms unless the numbers really are everything there is: the
eight marks above are a sample, so `stdev` gives `2.14` where `pstdev` gives
`2.0`.

## Whether two sets move together

| Function | Result |
| --- | --- |
| `covariance(xs, ys)` | How much the two vary together, in the product of their units |
| `correlation(xs, ys)` | Pearson's correlation, from `-1.0` to `1.0` |

```mars
hours = arr(1.0, 2.0, 3.0, 4.0);
marks = arr(2.0, 4.0, 7.0, 8.0);
out(stats.correlation(hours, marks));   // 0.9844951849708403
```

`1.0` means one rises exactly as the other does, `-1.0` that one falls as the
other rises, and near `0.0` that they are unrelated. Both arrays must be the
same length and hold at least two numbers, and `correlation` needs numbers that
are not all the same, since a flat set has no direction to compare.

Correlation is not cause: two things can move together because a third moves
them both.

## Accuracy

Sums are compensated (Neumaier's improvement on Kahan summation), so adding
many floats does not drift: `stats.sum(arr(0.1, 0.2, 0.3))` is `0.6`, where
adding them left to right gives `0.6000000000000001`.

Variance is computed in two passes, the mean first and then the squared
distances from it. The one-pass form that sums the squares and subtracts the
square of the sum loses every significant digit when the numbers are large and
close together.

The package is written in Marslang: [std/stats.mars](../../std/stats.mars).
