# Collections

Collections compare by identity with `==` and `!=`. Assignment shares the same
container. `.copy()` creates a deep copy, including nested containers, preserving
cycles and shared relationships inside the copy. The copy drops fixed status and
retains element-type restrictions.

## Arrays

Construct with `arr(value,...)`; `a(...)` is a compatibility alias.
Use `values (array[int]) = arr(1,2);` to restrict element types.

### Inspection

| Method | Result |
| --- | --- |
| `len()` | Element count |
| `is_empty()` | Whether the array is empty |
| `has(value)` | Whether an equal value is present |
| `iget(index)` | Element at a nonnegative index; invalid indices raise `RangeError` |
| `get(value)` / `rget(value)` | First/last matching index, or `-1` |

### Mutation

| Method | Effect and return value |
| --- | --- |
| `add(value)` | Append; return the array |
| `pop()` / `lpop()` | Remove and return first/last element; `null` if empty |
| `iremove(index)` / `remove(value)` | Remove by index/first match; return removed value (`null` if no match) |
| `modify(index,value)` / `change(old,new)` | Replace one position/all matches; return the array |
| `sort()` / `asort()` | Sort descending/ascending in place; return the array |
| `rev()` / `reverse()` | Reverse in place; return the array |

Array reversal mutates the array. [String reversal](strings.md) returns a new string.

### Slicing

`slice(start,end)` and `lenslice(start,length)` return new arrays. They follow the
same bounds and optional `reverse=true` rules as [string slicing](strings.md),
counting elements instead of characters. Invalid bounds raise `OutOfBoundsError`.

A slice retains element-type restrictions. It is a shallow selection: nested
objects remain shared. The new outer array is mutable; shared fixed elements
remain fixed. Use `.copy()` when an independent deep copy is needed.

## Sets

Construct with `set(value,...)`; `s(...)` is a compatibility alias. Duplicate values
are removed. `values (set[int]) = set(1,2);` restricts elements to integers.

| Method | Result |
| --- | --- |
| `len()` / `is_empty()` | Count / emptiness |
| `has(value)` | Membership boolean |
| `add(value)` | Insert; return the set |
| `remove(value)` | Whether a value was removed |
| `copy()` | Independent deep copy |

Set slicing and sorting are not implemented.

## Pairs

Construct with `pair(first,second)`; `p(...)` is a compatibility alias.
Read/write `.first` and `.second`. A pair is mutable unless fixed.

```mars
func m{
    entry (pair[string,int]) = pair("score",1);
    entry.second = 2;
    out(entry.first);
}
```

## Maps and dictionaries

`map()` and `dict()` create the same insertion-ordered key/value container.
Use `counts (map[string,int]) = map();` for key/value restrictions.

### Lookup and size

| Method | Result |
| --- | --- |
| `get(key)` | Stored value, or `null` if absent |
| `has(key)` | Whether the key exists, including keys mapped to `null` |
| `len()` / `is_empty()` | Entry count / emptiness |

### Mutation and traversal

| Method | Result |
| --- | --- |
| `set(key,value)` | Insert or replace; return the map |
| `remove(key)` | Whether an entry was removed |
| `keys()` / `values()` | New arrays in key insertion order; contained objects remain shared |
| `copy()` | Independent deep copy |

Updating an existing key does not move it. Removing and reinserting a key puts it
at the end. Iteration yields keys, not key/value pairs.

## Iteration and fixed values

`for (item,collection){...}` works for arrays, sets, and maps. Each loop snapshots
the entries or keys at the start; later mutations do not alter that visit list.
Pairs are not iterable.

`fixed` recursively prevents collection mutation through every alias. Direct
initialization from a fixed binding also makes the new binding fixed. Use an
explicit `fixed` declaration for clarity, or `.copy()` for a mutable deep copy.
