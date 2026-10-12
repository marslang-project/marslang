# rs-0.16.0 — Random numbers, regular expressions, and raw strings

Two new standard packages: [std.random](../api/std-random.md) for dice, cards,
shuffles, weighted picks, and repeatable seeds, and [std.regex](../api/std-regex.md)
for finding, checking, replacing, and splitting text by pattern. Raw strings,
`r"..."`, keep backslashes as they are written, so patterns read plainly.

## std.random

```mars
takepkg std.random;

func m{
    out(random.int_between(1, 6));                                         // a dice roll
    deck = arr("A", "K", "Q", "J", "10");
    random.shuffle(deck);
    out(random.sample(deck, 2));                                           // two different cards
    out(random.weighted_choice(arr("common", "rare", "epic"), arr(80, 15, 5)));
}
```

| Function | Result |
| --- | --- |
| `float()`, `float_between(low, high)` | A float, from `0.0` or `low` up to, but never, `1.0` or `high` |
| `int_between(low, high)`, `int_below(count)` | A whole number, every value equally likely |
| `bool()`, `chance(probability)` | `true` half the time, or with that probability |
| `choice`, `choices`, `sample` | One item; several, repeats allowed; several from different positions |
| `shuffle`, `shuffled` | A random order, in place or as a copy |
| `weighted_choice`, `weighted_choices` | Picks in proportion to weights |
| `gauss(mean, deviation)` | The normal distribution |
| `seed(seed)`, `Generator(seed)` | Repeatable numbers; a generator of its own |

After `random.seed(42)`, or a string seed such as `random.seed("level-1")`, a
program makes the same numbers on every run and on every platform: the tests
check exact sequences on Windows, macOS, and Linux, on x86 and ARM.
`random.Generator(seed)` gives one part of a program numbers of its own, so a
procedural world stays the same however many dice the rest of the game rolls.

The generator is xoshiro256++, Julia's default and the one behind Rust's
`SmallRng`, with Lemire's method for ranges, so no value is favoured. It is the
only native part; shuffling, sampling, weights, and the normal distribution
are Marslang, in `std/random.mars`. It is not for passwords or keys.

## std.regex

```mars
takepkg std.regex;

func m{
    out(regex.matches(r"\d{4}-\d{2}-\d{2}", "2026-10-12"));             // true

    date = regex.find_whole(r"(?<year>\d{4})-(?<month>\d{2})-(\d{2})", "2026-10-12");
    out(date.named("year"), date.group(3));                            // 2026 12

    for (tag, regex.find_all(r"#(\w+)", "#rust and #mars")){ out(tag.group(1)); }
    out(regex.replace(r"(\w+)@(\w+)", "ada@home", "$2:$1"));             // home:ada
    out(regex.split(r"\s*,\s*", "a , b,c"));                            // ["a", "b", "c"]
}
```

`matches`, `contains`, `find`, `find_all`, `find_whole`, `replace`,
`replace_first`, `replace_with` (a function makes each replacement), `split`,
and `escape`, with a `Match` for each match (its text, character positions,
numbered and named groups) and `Regex(pattern)` to check a pattern once and
pass it around. Patterns are compiled on first use and kept.

Matching is Rust's regex crate, which takes time in proportion to the text for
every pattern: no pattern can hang a program, so patterns and text from users
are safe. In return, backreferences (`\1`) and lookaround (`(?=...)`) are not
supported, and say so. A pattern with a mistake raises a `SyntaxError` on one
line that says where: `regex "ab(c" at character 3: unclosed group`.

Positions count characters, as `len` and `slice` do, so
`text.slice(m.start, m.end)` is always the match, even after emoji.

## Raw strings

```mars
pattern = r"\d+(\.\d+)?";      // the same as "\\d+(\\.\\d+)?"
folder = r"C:\Users\ada\new";  // \n is not a line break here
```

An `r` before the opening quote makes a raw string, in which a backslash is
only a backslash: `r"..."`, `r'...'`, and `r"""..."""` over several lines. In an
ordinary string a backslash before a character with no escape of its own stands
for that character, so `"\d"` is just `d`, and a pattern written that way looks
for the letter. See [raw strings](../api/strings.md#raw-strings).

The VS Code extension 0.3.5 colours raw strings, and no longer marks the
escapes `\b`, `\f`, `\v`, `\xHH`, and `\u{...}` as mistakes.

## Validation

`cargo test` passes 167 tests on Linux (20 unit, 146 execution, 1 memory) and
165 on Windows, including exact seeded sequences, fairness checks over
thousands of draws, regex positions around emoji and combining accents, and
every error message. Every push runs them on Windows, macOS, and Linux on x86_64
and ARM64, and runs the static Linux binary inside Alpine, Debian 11, Ubuntu
20.04, Rocky Linux 8, and Amazon Linux 2.
