# rs-0.17.0 — Errors that say where, f-strings, tests, and big numbers

The biggest release yet: runtime errors now show the line and the calls that
led there, strings can hold values with `f"..."`, programs can be tested with
`marslang test`, integers can be any size, and eight new standard packages
reach the network, the terminal, and the rest of the world.

## Errors say where

```text
error: RangeError: random.int_between needs low <= high, got 6 and 1
    at roll (dice.mars:4)
    at Game.play (game.mars:7)
    at m (game.mars:14)
```

Every uncaught error prints its line and the calls that led there, innermost
first, as `file:line`, which editors open with a click. Calls inside the
standard library are left out, and recursion collapses to one line with its
count. Compile errors such as `unknown name 'nope'` now say their line too,
and the VS Code extension underlines the exact name.

## f-strings

```mars
func m{
    name = "Ada";
    age = 36;
    out(f"{name} is {age}, and {age + 1} next year");   // Ada is 36, and 37 next year
}
```

Any expression goes between the braces; `{{` and `}}` are braces. A string
inside the braces uses the other quote: `f"{scores.get('ada')}"`.
See [f-strings](../api/strings.md#f-strings).

## Tests: std.testing and marslang test

`marslang test` runs every function named `test_...` in every `*_test.mars`
file, each on its own: a failure shows the line of the assertion and the next
test runs, and the command ends with status 1 when anything failed.
[std.testing](../api/std-testing.md) has `equal` (which compares containers by
contents), `not_equal`, `is_true`, `is_false`, `close`, `raises`, and `fail`.

## bigint: integers of any size

`bigint(2) ** 200`, `bigint("123456789012345678901234567890")`, and literals
beyond longint are a new kind, `bigint`, with every operator. Ints and longints
mix with it; floats need an explicit conversion; equal numbers of every kind are
the same map key. `json.parse` reads huge integers as bigints.
[std.bigint](../api/std-bigint.md) adds gcd, lcm, factorial, integer square
root, floor division, and bases 2 to 36.

## New packages

| Package | For |
| --- | --- |
| [std.http](../api/std-http.md) | Fetching pages, JSON APIs, and downloads over HTTPS, through proxies from the environment |
| [std.format](../api/std-format.md) | `fixed(3.14159, 2)`, `number(1234.5, 2)` is `"1,234.50"`, percentages, and tables that line up even with Chinese and emoji |
| [std.terminal](../api/std-terminal.md) | Colours and styles, progress bars, and the terminal's size, respecting `NO_COLOR` |
| [std.log](../api/std-log.md) | `log.info(...)` with levels and timestamps, to standard error or a file |
| [std.hash](../api/std-hash.md) | SHA-256, SHA-512, SHA-1, MD5, CRC-32 of text and files, and HMAC |
| [std.encoding](../api/std-encoding.md) | base64, hex, URL encoding, query strings, and bytes |
| [std.testing](../api/std-testing.md) | Assertions for `marslang test` |
| [std.bigint](../api/std-bigint.md) | Integer arithmetic on bigints |

## Smaller changes

- A statement can go on over several lines inside `( )` or `[ ]`, so long
  calls and arrays can be laid out; a bracket left open says where.
- The installers find the latest release without GitHub's API, whose limit of
  60 lookups an hour per address made installs fail on shared networks.
- Release binaries are stripped: about 30% smaller, even with HTTPS added.
- `types.family_name(RangeError)` names a family itself.

## Validation

`cargo test` passes 300 tests on Linux and 298 on Windows, including the new
packages' own test files; HTTP is tested against a local server. Every push
runs them on Windows, macOS, and Linux on x86_64 and ARM64, and runs the static
Linux binary inside Alpine, Debian 11, Ubuntu 20.04, Rocky Linux 8, and
Amazon Linux 2.
