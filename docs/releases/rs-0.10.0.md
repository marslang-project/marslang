# rs-0.10.0 — std.Error

The built-in error families now have a namespace of their own, and families are
raised by value everywhere, never by name as a string.

## std.Error

```mars
takepkg std.Error;

family ParseError(Error.Base){}

func m{
    run{ err(Error.RangeError, "too big"); } handle(Error.RangeError e){ out(e); }
    run{ x = 1 / 0; } handle(Error.Base e){ out(e); }       // RangeError: division by zero
}
```

[std.Error](../api/std-error.md) exports `Base`, `TypeError`, `RangeError`,
`OutOfBoundsError`, and `SyntaxError`. They are the interpreter's own families,
not copies, so a handler for `Error.RangeError` catches a division by zero, and
`Error.TypeError` is the same family as the bare `TypeError`, which keeps
working without the import.

## A family can extend one from a package

`family ParseError(Error.Base){}` and `family Timeout(net.NetworkError){}` now
work: a parent written as `alias.Family` is looked up in the packages the file
imports. Before, only families declared in the same file and the bare built-in
names could be parents.

## Families are raised, not called

`err(TypeError, "message")` raises; calling `TypeError("message")` now fails
with a `TypeError` that says to use `err`. Previously it failed with "TypeError
has no init and takes no arguments", and `TypeError()` with no arguments quietly
built an error without a message. A family you declare with an `init` still
constructs, and `err(instance)` raises it.

## Changed

- `rs.core.raise` takes a family, `core.raise(RangeError, "message")`, instead
  of its name as a string. It raises built-in families only; standard packages
  use it, and everything else uses `err`.
- A family prints as `<family TypeError>` rather than `<func TypeError>`.

## Validation

`cargo test` passes 114 tests (13 unit, 100 execution, 1 memory), with the new
`std.Error` cases among them, on Windows Rust and on WSL-native Rust.
