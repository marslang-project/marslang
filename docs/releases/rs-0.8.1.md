# rs-0.8.1 — Access, type-check, and padding fixes

A patch release fixing the four findings of the rs-0.8.0 review. `std.Decorator`
now also lists the markers it provides.

## Fixes

| Issue | Now |
| --- | --- |
| A private method taken as a value could be called from outside its family | Every call of a bound method is checked against the method running at that moment, including methods stored in fields or containers |
| A rejected insertion still converted and restricted the value being inserted | An insertion checks all of a container's restrictions as one plan and applies it only if every part passes; a map `set` checks key and value together |
| Nested unions rejected shared containers that a different choice would have accepted | Union choices are recorded while planning and retried depth-first when a later constraint conflicts (up to 64 attempts) |
| `pad_start`/`pad_end` could return fewer characters than requested | The padded result is measured; a fill that merges with its neighbours (a lone combining accent, one flag letter) raises `RangeError` |

## Changed: a container's restrictions must all hold

Every element must satisfy each of its container's restrictions as stored, so
conflicting restrictions now raise `TypeError` and change nothing:

```mars
x (array[int]) = arr(1);
y (array[float]) = x;      // TypeError: cannot satisfy both
```

Previously the alias converted `x`'s elements to floats even though `x` promised
`int`, leaving the array inconsistent with its own annotation.

## std.Decorator lists its markers

[std/Decorator.mars](../../std/Decorator.mars) now exports one value per marker,
so `@Decorator.NAME` is accepted only for a name the package provides, an unknown
marker reports the available list, and `out(Decorator.private)` prints `private`.
Markers that the interpreter does not apply yet still report "not implemented yet".

## Validation

`cargo test` passes 111 tests (13 unit, 97 execution, 1 memory) on Windows Rust and
on WSL-native Rust.
