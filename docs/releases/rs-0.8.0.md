# rs-0.8.0 — Private methods and reliability fixes

This release adds private methods through `std.Decorator` and fixes the six findings
of the rs-0.7.0 review, including two ways to crash the interpreter.

## Private methods

```mars
takepkg std.Decorator;

family Account{
    @Decorator.private
    func _audit(string action){ out("audit " + action); }

    @Decorator.subclass
    func _limit() => 100;

    func deposit(int amount){ me._audit("deposit"); }
}

family Savings(Account){
    func limit() => me._limit();     // allowed
}

func m{
    Account()._audit("x");           // TypeError: _audit is private to Account
}
```

- `std.Decorator` is a namespace of markers written above family methods; importing
  it makes `@Decorator.NAME` available (or `@alias.NAME` with an alias).
- `@Decorator.private`: only methods of the declaring family may call the method.
- `@Decorator.subclass`: methods of families inheriting from it may call it too.
- Calls and bound-method values from anywhere else raise `TypeError`. Mistakes such
  as a missing import, unknown markers, or `private` together with `subclass` are
  compile errors. `static`, `class`, and `overload` are planned.
- `std.containers` now marks its internal helper methods private.

## Fixes

| Issue | Now |
| --- | --- |
| `time.sleep(1e30)` crashed the interpreter | Raises a catchable `RangeError`; `then` blocks still run |
| An error whose message contained itself overflowed the stack when printed | Prints `<cycle>` |
| A panic inside any native Rust function crashed the interpreter | Becomes a catchable `Error` (defense in depth) |
| Shared containers could be partly changed by a failed type check | Every change is computed first and written together; failed alternatives and annotations change nothing |
| `strings.find`/`rfind` could return a position past the end | Searches match whole characters only |
| `math.gcd(INT_MIN, 1)` raised overflow | Returns 1; `gcd(INT_MIN, 0)` still raises overflow |
| `priority_queue.items()` was missing | Returns items in pop order without changing the queue |

### Changed: string searches match whole characters

`find`, `rfind`, `contains`, `starts_with`, `ends_with`, `split`, and `replace` in
`std.strings` only match at character boundaries, so a position from `find` always
works with `lenslice`. A combining accent inside a character is not found on its
own, and `"\r\n"` is one character, so `split(text, "\n")` does not split Windows
line endings; use `lines(text)`.

## Validation

`cargo test` passes 106 tests (13 unit, 92 execution, 1 memory) on Windows Rust and
on WSL-native Rust.

## Remaining work

The remaining decorators, opaque native handles, a bytes type, program arguments,
closures and function-typed parameters, `match`, file/JSON/random/regex packages,
the per-user package directory and installer, and async remain pending.
