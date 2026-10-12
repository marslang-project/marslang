# std.regex

Regular expressions: checking that text has a shape, finding the parts that
do, and replacing or splitting by pattern.

```mars
takepkg std.regex;

func m{
    out(regex.matches(r"\d{4}-\d{2}-\d{2}", "2026-10-12"));          // true

    found = regex.find(r"(\w+)@([\w.]+)", "write to ada@example.com");
    out(found.text, found.group(1), found.group(2));                // ada@example.com ada example.com

    out(regex.replace(r"\s+", "too    many   spaces", " "));        // too many spaces
    out(regex.split(r"\s*;\s*", "a; b ;c"));                        // ["a", "b", "c"]
}
```

Write patterns as raw strings, `r"..."`, in which a backslash is just a
backslash. In an ordinary string `\d` is the letter `d` and `\b` a backspace,
so `"\d+"` looks for the letter d; the same pattern there is `"\\d+"`.

| Function | Result |
| --- | --- |
| `matches(pattern, text)` | Whether the whole text matches |
| `contains(pattern, text)` | Whether the pattern matches anywhere in the text |
| `find(pattern, text)` | The first match, as a `Match`, or `null` |
| `find_all(pattern, text)` | Every match, left to right, as an array of `Match` |
| `find_whole(pattern, text)` | The match of the whole text, as a `Match`, or `null` |
| `replace(pattern, text, replacement)` | The text with every match replaced |
| `replace_first(pattern, text, replacement)` | The text with the first match replaced |
| `replace_with(pattern, text, f)` | The text with each match replaced by `f(match)` |
| `split(pattern, text)` | The pieces of text between matches |
| `escape(text)` | A pattern that matches the text exactly |
| `Regex(pattern)` | A pattern checked once, with all of the above as methods |

The pattern comes first. `matches` checks the whole text, for validating
input; `contains` and `find` look anywhere in it.

## Matches

A `Match` has:

| Member | Value |
| --- | --- |
| `text` | What matched |
| `start`, `end` | Where, as character positions: `text.slice(m.start, m.end)` is the match |
| `groups` | The text of each `( )` group in order, `null` for a group that took no part |
| `names` | A map from each named group to its text |
| `group(number)` | Group `number`, counting `(` from the left from `1`; `0` is the whole match |
| `named(name)` | The named group `(?<name>...)` |

```mars
takepkg std.regex;

func m{
    date = regex.find_whole(r"(?<year>\d{4})-(?<month>\d{2})-(?<day>\d{2})", "2026-10-12");
    out(date.named("year"), date.group(2), date.names);   // 2026 10 {"year": "2026", "month": "10", "day": "12"}

    for (tag, regex.find_all(r"#(\w+)", "#rust and #mars")){
        out(tag.group(1), tag.start);                    // rust 0, then mars 10
    }
}
```

Positions count characters, as `len` and `slice` do, so an emoji or an accented
letter before a match counts once. Patterns themselves work on Unicode code
points, as in other languages: `.` is one code point, so a character made of
several, such as `👨‍👩‍👧‍👦` or an `e` followed by a combining accent, takes more
than one `.`. A match that begins or ends inside such a character covers all of
it in `start` and `end`.

## Replacing

In a replacement, `$1` or `${1}` is the text of group 1, `${name}` that of a
named group, and `$$` a dollar sign. Write `${1}` when a letter, digit, or `_`
follows: `$1st` means a group named `1st`. A group that does not exist, or took
no part, is empty.

```mars
takepkg std.regex;

func capitalize(regex.Match word) => word.text.upper();

func m{
    out(regex.replace(r"(\w+)@(\w+)", "ada@home", "$2:$1"));              // home:ada
    out(regex.replace(r"(\d+)", "3 apples", "${1}0"));                    // 30 apples
    out(regex.replace_with(r"\b\w", "ada lovelace", capitalize));         // Ada Lovelace
}
```

`replace_with` calls a function with each `Match` and uses the string it
returns; anything else raises `TypeError`.

## Patterns

| Pattern | Matches |
| --- | --- |
| `.` | Any character but a line break |
| `\d`, `\w`, `\s` | A digit, a word character (letter, digit, `_`), whitespace |
| `\D`, `\W`, `\S` | Anything but those |
| `[abc]`, `[a-z]`, `[^0-9]` | One of, a range, none of |
| `\b` | A word boundary |
| `^`, `$` | The start and end of the text (of each line under `(?m)`) |
| `a*`, `a+`, `a?` | Zero or more, one or more, zero or one |
| `a{3}`, `a{2,5}`, `a{2,}` | Exactly 3, from 2 to 5, 2 or more |
| `a*?`, `a+?` | As few as possible |
| `a\|b` | Either |
| `(a)`, `(?<name>a)`, `(?:a)` | A group, a named group, a group that is not counted |
| `\p{Greek}`, `\p{Lu}` | A character of a Unicode script or category |

Flags go at the start of a pattern, or of a group:

| Flag | Effect |
| --- | --- |
| `(?i)` | Ignore case: `(?i)hello` matches `HeLLo` |
| `(?m)` | `^` and `$` match at the start and end of each line |
| `(?s)` | `.` matches line breaks too |
| `(?x)` | Spaces are ignored and `#` starts a comment, for long patterns |

`\d` and `\w` follow Unicode, so `\d` matches digits of every script, such as
`٣`; write `[0-9]` for the ASCII digits alone. The full syntax is that of
Rust's [regex crate](https://docs.rs/regex/latest/regex/#syntax).

## Regex

`regex.Regex(pattern)` checks the pattern straight away, so a mistake shows
where the pattern is written, not at its first use, and gives an object to pass
around. Its methods are the functions above without the pattern:

```mars
takepkg std.regex;

func m{
    word = regex.Regex(r"\w+");
    out(word.find_all("one two three").len());   // 3
    out(word.matches("one two"), word.pattern);  // false \w+
}
```

Every pattern, `Regex` or not, is compiled the first time it is used and kept,
so a pattern used in a loop is compiled once; the 64 used most recently are
kept.

## Errors

A pattern that is not valid raises `SyntaxError`, saying where:
`regex "ab(c" at character 3: unclosed group`. A pattern so large that its
compiled form would pass 10 MB raises `RangeError`. `group` with a number the
pattern has no group for raises `OutOfBoundsError`, and `named` with a name it
lacks raises `RangeError`.

## Speed, and what is not supported

Matching takes time in proportion to the length of the text, whatever the
pattern. Patterns that make other languages' engines run for hours on a short
string, such as `(a+)+$`, cannot slow `std.regex` down, so it is safe to match
patterns or text that come from users.

That guarantee rules out two features, which raise `SyntaxError` when used:
backreferences such as `\1` (a group matched again), and lookaround such as
`(?=...)` and `(?<!...)`. Use a group and check it afterwards instead:
`find_all(r"(\w+) (\w+)", text)` and compare `group(1)` with `group(2)`.

## How it works

Matching is native ([std/rs/regex.rs](../../std/rs/regex.rs)), by Rust's
[regex](https://docs.rs/regex) crate, which compiles a pattern to finite
automata rather than backtracking. `Match`, `Regex`, and `replace_with` are
Marslang, in [std/regex.mars](../../std/regex.mars).
