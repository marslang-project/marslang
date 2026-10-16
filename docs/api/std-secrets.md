# std.secrets

Random bytes fit for passwords, keys, and tokens: the operating system's
cryptographically secure random source, with the functions built on it
(hex and URL-safe tokens, whole numbers, choices, and passwords) and a
comparison that takes the same time whatever the strings hold.

```mars
takepkg std.secrets;

func m{
    out(secrets.token_hex(16).len());               // 32
    out(secrets.token_url(24).len());               // 32
    out(secrets.password(16).len());                // 16
    out(secrets.int_below(6));                      // 0 to 5, each a sixth of the time
    out(secrets.choice(arr("red", "green")));       // red or green
}
```

Each run gives different output. Nothing here can be seeded, and that is the
point: no program can make these numbers repeat.

| Function | Result |
| --- | --- |
| `bytes(n)` | `n` random bytes, each an int from `0` to `255`; `n` from `0` to `65536` |
| `token_hex(n)` | `n` random bytes as `2n` lowercase hex digits |
| `token_url(n)` | `n` random bytes as URL-safe base64 with no `=` padding: `-` and `_` in place of `+` and `/` |
| `int_below(count)` | A whole number from `0` up to, but not, `count`, every value equally likely |
| `choice(items)` | One item of an array, every item equally likely; an empty array raises `RangeError` |
| `password(length)` | A password of `length` characters, from `1` to `1024`: letters, digits, and symbols |
| `same(a, b)` | Whether two strings are equal, comparing every character |

## Choosing: secrets or random

For passwords, tokens, keys, session IDs, and anything else an attacker must
not guess, always use `std.secrets`. For simulations, games, sampling, and test
data, use [std.random](std-random.md): it is faster, and it repeats its numbers
from a seed, which is what makes it unsafe for secrets. Its generator can be
worked out from enough of its output, so a token made with `std.random` can be
guessed.

A random ID that must not clash is different again. For that, a UUID version 4
from [std.uuid](std-uuid.md) is the usual choice: its 122 random bits make a
clash so unlikely that you need not check for one.

## Tokens

`token_hex` and `token_url` both draw `n` bytes, so 16 bytes is 128 bits, the
usual size for a token that must not be guessed. Pick the form the token
travels in: hex for logs and plain text, `token_url` for links, filenames, and
anything in a URL.

```mars
takepkg std.secrets;

func m{
    out(secrets.token_hex(0).len());          // 0: an empty token
    out(secrets.token_url(16).len());         // 22: ceil(4 * 16 / 3) characters
    out(secrets.bytes(4).len());              // 4 ints, each from 0 to 255
}
```

## Whole numbers and choices

`int_below(count)` gives a number from `0` to `count - 1`, with no bias. A
program that takes random bytes modulo `count` favours some values when `count`
is not a power of 256. `int_below` instead draws only as many bytes as it
needs, and throws away any value at or above the largest multiple of `count`
that fits in the range of those bytes, so each value comes up equally often.

```mars
takepkg std.secrets;

func m{
    out(secrets.int_below(1));                // 0, always
    out(secrets.choice(arr(7)));              // 7, always
    deck = arr("A", "K", "Q", "J");
    out(deck[secrets.int_below(deck.len())]); // one of the four, equally likely
}
```

`count` may be an int or a longint. The result is an int for an int count and
a longint for a longint one, so `int_below(9223372036854775807)` covers every
longint below the maximum.

## Passwords

`password(length)` draws characters from a 73-character alphabet: the letters
`a` to `z` and `A` to `Z`, the digits `0` to `9`, and the symbols
`!@#$%^&*-_=+`.

From length 4 up, a password has at least one lowercase letter, one uppercase
letter, one digit, and one symbol. The generator draws a password and draws
again until it has all four kinds, so every password that qualifies is equally
likely. Lengths 1 to 3 cannot hold all four kinds, so they are drawn with none
guaranteed.

```mars
takepkg std.secrets;

func m{
    pin = secrets.password(4);                // always has all four kinds
    out(pin.len());                           // 4
    out(secrets.password(1024).len());        // 1024, the longest allowed
}
```

## Comparing

`same(a, b)` answers whether two strings are equal, and compares every
character before it answers. A comparison that stops at the first difference
can tell an attacker how much of a secret they guessed right. Use `same` to
check a token or an HMAC against the one someone sent you.

```mars
takepkg std.secrets;

func m{
    token = secrets.token_hex(8);
    out(secrets.same(token, token));          // true
    out(secrets.same(token, "0000"));         // false
}
```

## Errors

| Call | Error |
| --- | --- |
| `bytes(n)`, `token_hex(n)`, `token_url(n)` with `n` below `0` or above `65536` | `RangeError` |
| `int_below(count)` with `count` of `0` or less | `RangeError` |
| `choice(items)` with an empty array | `RangeError` |
| `password(length)` with `length` below `1` or above `1024` | `RangeError` |
| A count that is not a whole number, or a `same` argument that is not a string | `TypeError` |

Each message names the function and gives the value it was given.

## Behaviour

Every call draws fresh bytes from the operating system. Nothing is cached, and
no seed is accepted. A program that needs the same token twice must keep it.

`bytes` returns ints, not text, so raw bytes are never misread as characters.
`token_hex` and `token_url` return text in a fixed alphabet each.

## How it works

The random bytes come from the `getrandom` crate, version 0.2, which asks the
operating system: `getrandom(2)` on Linux, `BCryptGenRandom` on Windows, and
`getentropy` on macOS. That is the one native function
([std/rs/secrets.rs](../../std/rs/secrets.rs)). Version 0.2 is already in the
dependencies through `ring`; later versions link through `raw-dylib` on Windows,
which the build cannot use.

Everything else is Marslang, in [std/secrets.mars](../../std/secrets.mars):
hex and URL-safe text, the rejection sampling for `int_below`, the choices, the
password rules, and the error messages. `int_below` does its arithmetic with
bigints, so the threshold for a longint count is exact.
