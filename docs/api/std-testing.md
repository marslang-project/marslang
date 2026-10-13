# std.testing

Tests for your own code: assertions in [std.testing](../../std/testing.mars),
run by `marslang test`.

Put the tests for `calc.mars` in `calc_test.mars` beside it. A test is a
top-level function named `test_...` that takes no parameters:

```mars
takepkg calc;
takepkg std.testing;

func test_add(){
    testing.equal(calc.add(2, 3), 5);
}

func test_half(){
    testing.equal(calc.half(7), 3);
}
```

```text
marslang test
calc_test.mars
  ok    test_add
  FAIL  test_half
        AssertionError: expected 3, got 3.5
            at test_half (calc_test.mars:9)

1 passed, 1 failed in 0.01s
```

## Running tests

| Command | Runs |
| --- | --- |
| `marslang test` | Every `*_test.mars` file in this folder and the folders inside it |
| `marslang test path/to/folder` | Every `*_test.mars` file under that folder |
| `marslang test calc_test.mars` | That one file |

Each file loads as a program does, running its top-level statements once;
its `m`, if it has one, is not called. Then each test runs on its own, in the
order written: one that fails is reported, with the line where it failed, and
the next test runs. What a passing test prints is hidden; a failing test's
output is shown under it. `marslang test` ends with status 1 when any test
failed or any file could not run, so scripts and CI can rely on it.

Folders whose names start with `.`, and `target`, `node_modules`, and
`marslang_pkgs`, are not searched. A test file finds packages as any program
does, from its own folder, which is why it goes beside the code it tests.

## Assertions

| Function | Raises `AssertionError` unless |
| --- | --- |
| `equal(actual, expected)` | `actual` equals `expected` |
| `not_equal(actual, unexpected)` | `actual` does not equal `unexpected` |
| `is_true(value)` | `value` is `true` |
| `is_false(value)` | `value` is `false` |
| `close(actual, expected, tolerance)` | the numbers are at most `tolerance` apart |
| `raises(Family, f)` | calling `f()` raises that family, or one inheriting from it |
| `fail(message)` | never: it always raises, with `message` |

`equal` compares arrays, maps, sets, and pairs by their contents, so
`testing.equal(algorithms.sorted(arr(3, 1, 2)), arr(1, 2, 3))` passes, where
`==` would ask whether two arrays are the same one. Numbers compare by value,
so `1` equals `1.0`; everything else compares with `==`. Its message shows
both values, strings in quotes: `expected 3, got "3"`.

Floats are rarely exact, so compare them with `close`:
`testing.close(0.1 + 0.2, 0.3, 1e-9)`.

`raises` returns the error, to check its message too:

```mars
takepkg std.random;
takepkg std.testing;

func test_empty_choice(){
    func choose() => random.choice(arr());
    e = testing.raises(RangeError, choose);
    testing.equal(e.message, "random.choice needs at least one item");
}
```

`testing.AssertionError` inherits from `Error`, so a test can also check
something itself and raise it, or catch it with
`handle(testing.AssertionError e)`.
