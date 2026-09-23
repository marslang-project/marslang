# rs-0.13.0 — Algorithms, statistics, and memory

Three new standard packages, all reachable now that functions can be passed
around: sorting and searching in [std.algorithms](../api/std-algorithms.md),
descriptive statistics in [std.stats](../api/std-stats.md), and object identity
in [std.memory](../api/std-memory.md). `@Decorator.private` also applies to a
package's own functions and families.

## std.algorithms

```mars
takepkg std.algorithms;

func m{
    people = arr(Person("Ada", 36), Person("Grace", 45), Person("Alan", 41));
    for (p, algorithms.sort_by(people, func(Person p) => p.age)){ slout(p.name + " "); }
    out("");                                                          // Ada Alan Grace
    out(algorithms.transform(arr(1, 2, 3), func(int x) => x * x));    // [1, 4, 9]
    out(algorithms.counts(arr("fig", "pear", "fig")));                // {"fig": 2, "pear": 1}
}
```

`sorted`, `sort_by`, and `sort_with` are stable and O(n log n) for every input.
`sort_by` sorts the keys with the interpreter's own sort and regroups, so the
comparisons stay native and it runs about five times faster than a comparator
sort. `sort_with`, which must run Marslang code for each comparison, finds runs
that are already ordered, the way [Timsort](https://en.wikipedia.org/wiki/Timsort)
does, and insertion-sorts below 16 items, the threshold
[pdqsort](https://github.com/orlp/pdqsort) and introsort use: an array that is
already in order costs one pass, and no input drops it to O(n²).

Also `binary_search`, `lower_bound`, `upper_bound`, `transform`, `keep`,
`fold`, `min_by`, `max_by`, `any_of`, `all_of`, `unique`, `counts`, and
`group_by`.

## std.stats

```mars
takepkg std.stats;

func m{
    marks = arr(2, 4, 4, 4, 5, 5, 7, 9);
    out(stats.mean(marks), stats.median(marks), stats.mode(marks));   // 5 4.5 4
    out(stats.stdev(marks));                                          // 2.138089935299395
    out(stats.quantile(marks, 0.25));                                 // 4
}
```

`mean`, `median`, `mode`, `quantile`, `sum`, `variance`, `pvariance`, `stdev`,
`pstdev`, `covariance`, and `correlation`. Sums are compensated, so
`stats.sum(arr(0.1, 0.2, 0.3))` is `0.6` where adding left to right gives
`0.6000000000000001`. Variance is computed in two passes, which keeps its
digits where the one-pass form loses them, and quantiles interpolate as NumPy
and R type 7 do. Every result in the tests is checked against Python's
`statistics` module and NumPy.

## std.memory

```mars
takepkg std.memory;

func m{
    a = arr(1, 2);
    b = a;
    out(memory.same(a, b), memory.same(a, arr(1, 2)));   // true false
    out(memory.pointer(a));                              // array@0x1e248d94d30

    cycle = arr(); cycle.add(cycle); cycle = null;
    out(memory.collect());                               // 1
}
```

`address` and `pointer` say where a value lives, `same` whether two names hold
one object, `refs` how many references keep it alive, and `collect` runs the
cycle collector and reports what it freed. Numbers, booleans, and `null` live
inside the value itself and have no address, which the error says.

There is no `alloc` or `free`: Marslang frees values by itself, so manual
allocation would teach C's habits without C's reason for them. An address is
for telling objects apart and for debugging, never for reaching back into
memory, and it means nothing between two runs.

## Private package declarations

`@Decorator.private` used to be a compile error above a function or family
outside a family. It now means the one thing it can mean there: the package
does not export it.

```mars
takepkg std.Decorator;

@Decorator.private
func check(array values){ ... }

func mean(array values){ check(values); ... }   // fine, same package
```

An importer asking for `stats.check` gets `stats has no member 'check'`. A
leading underscore still does the same, and `marslang symbols` now reports each
function's real access. `@Decorator.subclass` stays methods-only.

## Validation

`cargo test` passes 131 tests (13 unit, 117 execution, 1 memory) on Windows
Rust and on WSL-native Rust. Every example on the new documentation pages was
run against this build.
