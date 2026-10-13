# std.encoding

Base64, hex, and URL encoding, query strings, and raw bytes. Text is always
encoded as its UTF-8 bytes.

```mars
takepkg std.encoding;

func m{
    out(encoding.base64("foobar"));                         // Zm9vYmFy
    out(encoding.from_base64("Zm9vYmFy"));                  // foobar
    out(encoding.hex("hé"));                                // 68c3a9
    out(encoding.bytes("hé"));                              // [104, 195, 169]
    out(encoding.url("a b/c"));                             // a%20b%2Fc

    q = encoding.query(map().set("q", "two words").set("page", 2));
    out(q);                                                 // q=two%20words&page=2
    out(encoding.from_query("?" + q));                      // {"q": "two words", "page": "2"}
}
```

| Function | Result |
| --- | --- |
| `base64(text)` | The text's UTF-8 bytes as standard base64, padded with `=` |
| `from_base64(text)` | Text from base64 (padding optional, whitespace skipped) |
| `base64_url(text)` | URL-safe base64 (`-` and `_`), no padding |
| `from_base64_url(text)` | Text from URL-safe base64 |
| `hex(text)` | The UTF-8 bytes as lowercase hex |
| `from_hex(text)` | Text from hex, in either case |
| `bytes(text)` | The UTF-8 bytes, as an array of ints 0 to 255 |
| `from_bytes(array)` | Text from an array of UTF-8 bytes |
| `base64_bytes(array)` | An array of bytes as base64, for binary data |
| `from_base64_bytes(text)` | Base64 as an array of bytes |
| `url(text)` | Percent-encoding for one URL component |
| `from_url(text)` | Text from percent-encoding |
| `query(map)` | A map as `a=1&b=two%20words` |
| `from_query(text)` | A query string as a map of text to text |

## Base64

`base64` and `from_base64` use the standard RFC 4648 alphabet. Encoding pads
with `=`. Decoding is forgiving, since pasted data often is:

- missing padding is accepted (`Zm8` and `Zm8=` both decode to `fo`);
- spaces, tabs, and line breaks are skipped, so data wrapped over lines works;
- `=` may only appear at the end, at most twice, and so that the total length is
  a multiple of 4;
- leftover non-zero bits in the last character are ignored.

`base64_url` and `from_base64_url` use the URL-safe alphabet (`-` and `_` for
`+` and `/`) and write no padding; decoding accepts it anyway. Each variant
rejects the other's characters and says which function to use.

Text goes in and comes out as UTF-8. If decoded bytes are not valid UTF-8
text, `from_base64` raises `SyntaxError` suggesting `from_base64_bytes`, which
returns an array of ints instead:

```mars
takepkg std.encoding;

func m{
    data = encoding.from_base64_bytes("AP8Q");
    out(data);                          // [0, 255, 16]
    out(encoding.base64_bytes(data));   // AP8Q
}
```

## Hex and bytes

`hex` writes two lowercase digits per UTF-8 byte; `from_hex` reads either case
and needs an even number of digits. `bytes` and `from_bytes` convert between
text and arrays of ints, so you can inspect or build the bytes yourself.
Values outside 0 to 255 raise `RangeError`, other types `TypeError`, each naming
the byte (counting from 1).

## URLs

`url` encodes one component, a path segment or a query value, so `/`, `?`, `&`,
`=`, `#`, and `+` are all escaped. `A-Z a-z 0-9 - . _ ~` stay as they are;
everything else becomes `%XX` for each UTF-8 byte, in uppercase hex.
`from_url` decodes `%XX` in either case. It leaves `+` as `+`, because a plus
means a space only in query strings, as in browsers.

## Query strings

`query` writes keys in the map's order, turning keys and values into text with
`string()` and percent-encoding both. `from_query` is its inverse:

```mars
takepkg std.encoding;

func m{
    out(encoding.from_query("?q=two+words&page=2&debug&page=3"));
    // {"q": "two words", "page": "3", "debug": ""}
}
```

- A leading `?` is ignored.
- `+` means a space; `%2B` is a real plus.
- A key without `=` has the value `""`; empty pieces (`a&&b`) are skipped.
- A repeated key: the last value wins, and the key keeps the position of its
  first appearance. Values are always text; convert with `int()` and the like.

## Errors

Malformed input raises `SyntaxError`. Each message names the function and the
character position, counted from 1, with what was found:

```
encoding.from_base64: character 3 is "!" which is not in the base64 alphabet
encoding.from_hex: odd number of hex digits (3); every byte needs two
encoding.from_url: bad percent-escape at character 3 ("%4")
encoding.from_query: bad percent-escape at character 7 ("%zz")
```

Bytes that decode to invalid UTF-8 also raise `SyntaxError`, giving the byte
number. Passing the wrong kind of value raises `TypeError`.

## How it works

Marslang strings are grapheme clusters with no byte access, so the byte-level
work is native Rust (`std/rs/encoding.rs`, hand-written, no dependencies): text
to UTF-8 bytes and back, the base64 and hex tables, and percent-encoding and
decoding. `std/encoding.mars` is the documented surface plus the query-string
functions, written in Marslang on top of the native percent-encoding: it splits
on `&` and `=`, and decodes each piece with `+` as a space. Positions in errors
count characters, so for text with combining accents they can differ slightly
from `len()`.
