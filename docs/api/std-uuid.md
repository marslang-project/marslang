# std.uuid

UUIDs, the 128-bit IDs of [RFC 9562](https://www.rfc-editor.org/rfc/rfc9562):
random ones (version 4), time-ordered ones (version 7), a check that text is
a UUID, and the version a UUID carries.

```mars
takepkg std.uuid;

func m{
    out(uuid.version(uuid.v4()));                  // 4
    out(uuid.version(uuid.v7()));                  // 7
    out(uuid.valid("not a uuid"));                 // false
    out(uuid.nil());                               // 00000000-0000-0000-0000-000000000000
}
```

Every UUID is text in the 8-4-4-4-12 form, such as
`5b1e9c2a-7d40-4f3e-9a61-0c8d2e7f4b13`: 32 hex digits in five groups.

| Function | Result |
| --- | --- |
| `v4()` | A random UUID, lowercase: 122 random bits, version 4 |
| `v7()` | A time-ordered UUID, lowercase: the Unix time in milliseconds, then random bits, version 7 |
| `valid(text)` | Whether `text` is a UUID in 8-4-4-4-12 hex form, in either case |
| `version(text)` | The version digit of a UUID, as an int |
| `nil()` | The nil UUID, `00000000-0000-0000-0000-000000000000` |

## Choosing a version

`v4()` makes a good random ID: a key, a session ID, or a file name that must
not clash with another. Its 122 random bits make a repeat so unlikely that
checking for one is not needed.

`v7()` suits IDs that should sort in the order they were made, such as the
keys of a table or the IDs in a log. Its first 48 bits are the time in
milliseconds, so a UUID made later sorts after one made earlier, and as text
too, since the form is lowercase throughout.

`nil()` is a placeholder for "no ID yet": no generated UUID is all zeros.

```mars
takepkg std.uuid;
takepkg std.time;

func m{
    first = uuid.v7();
    time.sleep(0.002);                  // a later millisecond
    out(first < uuid.v7());             // true
}
```

## Behaviour

`v4` and `v7` return lowercase text. `valid` accepts either case, so an ID
copied from an upper-case source still checks. It checks the shape only, not
the version or variant bits, so `valid` is true for a UUID of any version, and
`version` says which.

UUIDs made in the same millisecond are not guaranteed to sort in the order
they were made: their random bits decide between them. Across milliseconds the
order holds, unless the system clock is set back between two calls.

## Errors

| Call | Error |
| --- | --- |
| `version(text)` with text that is not a UUID in 8-4-4-4-12 hex form | `SyntaxError`, naming the text |
| `valid(text)` or `version(text)` with a value that is not a string | `TypeError` |

## How it works

Everything is Marslang, in [std/uuid.mars](../../std/uuid.mars). The random
bytes come from [std.secrets](std-secrets.md), which draws them from the
operating system. Marslang has no bitwise operators, so the version and variant
bits are set by arithmetic on the byte values: byte 6 becomes its value modulo
16 plus 64 for version 4 (plus 112 for version 7), and byte 8 becomes its value
modulo 64 plus 128 for the variant `10`. A version 7 UUID's first six bytes are
its millisecond time, most significant first, from `std.time`.
