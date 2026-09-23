# std.algorithms

Sorting, searching, and the passes most programs write over and over.

```mars
takepkg std.algorithms;

family Task{ func init(string name, int cost){ me.name = name; me.cost = cost; } }

func m{
    tasks = arr(Task("write", 3), Task("fix", 1), Task("ship", 2));
    for (task, algorithms.sort_by(tasks, func(Task t) => t.cost)){ slout(task.name + " "); }
    out("");                                     // fix ship write
    out(algorithms.transform(arr(1, 2, 3), func(int x) => x * x));   // [1, 4, 9]
}
```

Every function returns a new array and leaves its argument unchanged. Functions
are passed as values, so their parameter type is `Function`; see
[functions as values](language.md#functions-as-values).

## Sorting

| Function | Result |
| --- | --- |
| `sorted(items)` | A sorted copy, smallest first |
| `sort_by(items, key)` | A copy sorted by what `key` returns for each item |
| `sort_with(items, compare)` | A copy sorted by `compare(a, b)`: negative, `0`, or positive |

All three are O(n log n) and stable: items that tie keep the order they had.

```mars
algorithms.sort_by(people, func(Person p) => p.age);
algorithms.sort_with(tasks, func(any a, any b) => b.cost - a.cost);   // largest first
```

`sort_by` keys must all be numbers or all be strings; anything else raises
`TypeError`. `sort_with` takes any order you can write, including one over
families, which the `<` operator does not compare.

### How they sort

`sorted` and `sort_by` hand the work to the interpreter's own sort, written in
Rust: `sort_by` sorts the keys, then puts each item back under its key. Only
`sort_with` has to run Marslang code for every comparison, so it uses a merge
sort that first finds the runs that are already in order, extends short ones by
insertion below 16 items, and merges them, the way
[Timsort](https://en.wikipedia.org/wiki/Timsort) does and with the small-array
threshold [pdqsort](https://github.com/orlp/pdqsort) and introsort use. Sorting
an array that is already in order therefore costs one pass, and no input makes
it fall to O(n²) as a plain quicksort would.

Prefer `sort_by` when one value decides the order: it is about five times faster
than `sort_with` for the same array, because the comparisons stay native.

## Searching a sorted array

| Function | Result |
| --- | --- |
| `binary_search(items, value)` | The index of `value`, or `-1` |
| `lower_bound(items, value)` | The first index where `value` could be inserted |
| `upper_bound(items, value)` | The last such index: the first item greater than `value` |

Each halves the range at every step, so it looks at about log2(n) items instead
of all of them. They assume the array is already sorted; on an unsorted one the
answer is meaningless, not an error. `lower_bound` and `upper_bound` are the
same index when the value is absent, and differ by how many copies are there.

## Passes over a collection

| Function | Result |
| --- | --- |
| `transform(items, f)` | What `f` returns for each item, in order. Python's `map` |
| `keep(items, test)` | The items `test` is true for. Python's `filter` |
| `fold(items, start, f)` | One value: `start`, then `f(carried, item)` each time. Python's `reduce` |
| `min_by(items, key)`, `max_by(items, key)` | The item with the smallest or largest key, or `null` when empty |
| `any_of(items, test)`, `all_of(items, test)` | Booleans; both stop at the item that decides the answer |
| `unique(items)` | The items without repeats, keeping the first of each |
| `counts(items)` | A map from each item to how many times it appears |
| `group_by(items, key)` | A map from each key to the array of items with that key |

```mars
words = arr("pear", "fig", "plum", "fig");
out(algorithms.counts(words));                                  // {"pear": 1, "fig": 2, "plum": 1}
out(algorithms.group_by(words, func(string w) => w[0]));        // {"p": ["pear", "plum"], "f": ["fig", "fig"]}
out(algorithms.fold(words, "", func(string a, string b) => a + b.slice(0, 1)));   // pfpf
```

`unique` and `counts` compare items the way `set` and `map` do, so numbers and
strings work by value, while two arrays or two family instances with the same
contents count as different items.

The package is written in Marslang: [std/algorithms.mars](../../std/algorithms.mars).
