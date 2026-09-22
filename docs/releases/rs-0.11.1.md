# rs-0.11.1 — A documented standard library

Every public function, family, and method of the standard library now has a
docstring, and `marslang symbols` describes the packages a program imports, so
an editor can explain `math.sqrt` as well as your own names.

## Docstrings in the standard library

`std.math`, `std.strings`, `std.containers`, `std.types`, and `std.time` carry
111 docstrings between them, written from their reference pages:

```mars
takepkg std.math;

func m{
    out(math.sqrt(2.0));   // hover: func math.sqrt([int,longint,float] x)
}                          //        Square root, as a float. A negative x raises RangeError.
```

A test now fails if a public standard-library declaration has no docstring, so
new ones cannot be added without one.

## marslang symbols describes imported packages

The JSON gains `packages`: for each `takepkg`, the alias the program uses, the
package's name, and its public functions, families with their public methods,
and `fixed`/`hot` values with their types. Parameter names are shown as
written. A variable made by a package's family, such as
`s = containers.stack();`, is known to be a `containers.stack`.

## Validation

`cargo test` passes 118 tests (13 unit, 104 execution, 1 memory) on Windows
Rust and on WSL-native Rust.
