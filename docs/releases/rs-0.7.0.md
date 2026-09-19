# rs-0.7.0 — Error handling, cycle collection, and new standard packages

This release adds error handling, fixes the five findings of the rs-0.6.0
interpreter review (including a memory leak), and adds four standard packages.

## Error handling

```mars
family ParseError(Error){}

func m{
    run{
        err(ParseError, "empty input");
    } handle(ParseError e){
        out(e);                     // ParseError: empty input
    } handle([TypeError, RangeError] e){
        out(e.message);
    } then{
        out("always runs");
    }
}
```

- Errors are families. `Error` is the base; `TypeError`, `RangeError`,
  `OutOfBoundsError`, and `SyntaxError` inherit from it. Declare your own with
  `family Name(Error){}`. Errors have a `message` field and print as `Name: message`.
- `err(Family, message)` raises; `err(e)` raises a caught error again.
- `handle(Type e)`, `handle([T1, T2] e)`, and `handle(T1, T2)` (no name). The first
  matching handler runs; unmatched errors continue outward. Handlers also catch the
  interpreter's own errors, such as division by zero, and package errors by
  `alias.Family`.
- `then` always runs: after the body or a handler, while an error propagates, and on
  early `ret`, `break`, or `continue`.
- `lasterr()` returns the most recently handled error, or `null`.

## Memory and interpreter fixes

- **Cycles are reclaimed.** A cycle collector frees unreachable self-referencing
  data (an array containing itself, instances pointing at each other) while the
  program runs and when it ends. Previously every such cycle leaked.
- Family type annotations accept inherited families and compare declarations, not
  names: a parameter typed `Parent` accepts a `Child`, and same-named families from
  different packages are distinct. `alias.Family` names a package's family.
- Union annotations no longer change a value while trying an alternative that fails.
- `obj.f(args)` selects `f` before evaluating the arguments.
- Output is flushed before `in()`/`inln()` read, so prompts appear; on a terminal each
  `out`/`slout` appears immediately. Output errors are reported.
- The `any` type annotation accepts every value: `func push(any item)`.

## Standard packages

| Package | Contents |
| --- | --- |
| `std.containers` | `stack`, `queue`, `deque`, `priority_queue` (lowest priority first, ties in push order); written in Marslang |
| `std.math` | Now 51 functions: adds `gcd`, `lcm`, `is_even`, `is_odd`, `div_floor`, `div_ceil`, `factorial`, `perm`, `comb`, keeping the arguments' integer kind |
| `std.strings` | split, join, lines, trim, find, replace, upper/lower, `repeated`, padding; positions count characters |
| `std.types` | `kind`, `family_name`, `is_instance`, `is_number` |
| `std.time` | `now`, `monotonic`, `sleep` |

`std.strings` is named so that `takepkg` does not hide the built-in `string()`
conversion. Native helpers live in `std/rs/string.rs` and `std/rs/time.rs`;
`rs.core` gains `family_name` and `is_instance`.

## Validation

`cargo test` passes 98 tests (12 unit, 85 execution, 1 memory) on Windows Rust and
on WSL-native Rust. The memory test measures allocations: no bytes remain after a
run, and peak memory stays flat as a program creates more garbage cycles.

## Remaining work

Opaque native handles, a bytes type, program arguments, closures and function-typed
parameters, decorators, `match`, file/JSON/random/regex packages, the per-user
package directory and installer, and async remain pending.
