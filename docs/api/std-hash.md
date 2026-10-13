# std.hash

Checksums and message digests of text and files, and the HMAC keyed digest.
Use them to check a download against the digest its publisher printed, to
notice that a file has changed by accident, and to check that a request or
webhook was signed with a key you hold.

```mars
takepkg std.hash;

func m{
    out(hash.sha256("abc"));         // ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad
    out(hash.crc32("123456789"));    // cbf43926
    out(hash.md5(""));               // d41d8cd98f00b204e9800998ecf8427e
}
```

Every digest is returned as lowercase hex text. Text is hashed as its UTF-8
bytes, so the same text gives the same digest on every system.

| Function | Result |
| --- | --- |
| `sha256(text)` | The SHA-256 digest, 64 hex digits |
| `sha512(text)` | The SHA-512 digest, 128 hex digits |
| `sha1(text)` | The SHA-1 digest, 40 hex digits; broken for security, see below |
| `md5(text)` | The MD5 digest, 32 hex digits; broken for security, see below |
| `crc32(text)` | The CRC-32 checksum, 8 hex digits |
| `file(path, algorithm)` | The digest of a file; `algorithm` is `"sha256"`, `"sha512"`, `"sha1"`, `"md5"`, or `"crc32"` |
| `hmac_sha256(key, text)` | The HMAC-SHA-256 of text under key, 64 hex digits |
| `same(a, b)` | Whether two strings are equal, comparing every byte |

## Files

`file` reads a file in pieces and returns the digest of all of it. A file
larger than memory works, and so does a binary file: the bytes are what is
hashed, not text.

```mars
takepkg std.file;
takepkg std.file.path;
takepkg std.hash;

func check(string dir){
    file.write(path.join(dir, "notes.txt"), "abc");
    out(hash.file(path.join(dir, "notes.txt"), "sha256"));   // ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad
    out(hash.file(path.join(dir, "notes.txt"), "crc32"));    // 352441c2
}

func m{
    file.with_temp_dir(check);
}
```

## Keyed digests and comparing

`hmac_sha256` mixes a secret key into the digest, so only someone holding the
key can produce it. A webhook or API request signed this way can be checked
by computing the same HMAC and comparing. Use `same` for that comparison: it
looks at every byte, so the time it takes does not reveal where two
signatures first differ.

```mars
takepkg std.hash;

func m{
    key = "shared secret";
    tag = hash.hmac_sha256(key, "amount=10&to=ada");
    out(tag);                                                                        // c0b5584cb21c1d67d32174726994f47e7af4d9e98a8cee568ff852f6c8992455
    out(hash.same(tag, hash.hmac_sha256(key, "amount=10&to=ada")));                 // true
    out(hash.same(tag, hash.hmac_sha256(key, "amount=99&to=ada")));                 // false
}
```

`same` also works on any two strings, such as a digest from a download page
and the one you computed. Strings of different lengths are never equal, and
the length is the one thing it does not hide.

## Errors

An unknown algorithm raises `RangeError` before the file is opened, and
names the ones there are:

```mars
takepkg std.hash;
takepkg std.file;

func m{
    run{ hash.file("missing.bin", "sha256"); } handle(file.NotFoundError e){ out(lasterr().message); }
    // missing.bin: no such file or directory
    run{ hash.file("notes.txt", "sha3"); } handle(RangeError e){ out(lasterr().message); }
    // hash.file needs an algorithm of "sha256", "sha512", "sha1", "md5", or "crc32", got "sha3"
    run{ hash.file(".", "md5"); } handle(file.IsDirectoryError e){ out(lasterr().message); }
    // .: is a directory
}
```

A file that cannot be read raises a family from `std.file`, so a program that
already handles those errors needs no change:

| Failure | Family |
| --- | --- |
| Nothing at the path, or a folder on the way is missing | `file.NotFoundError` |
| The system refuses to read it | `file.PermissionError` |
| The path is a directory | `file.IsDirectoryError` |
| A part of the path is a file | `file.NotDirectoryError` |
| Anything else | `file.FileError` |

Every one is a `file.FileError`, so `handle(file.FileError e)` catches them
all, and every message starts with the path. Text that is not a string raises
`TypeError`.

## Behaviour

- Text is hashed as its UTF-8 bytes. `sha256("é")` hashes two bytes, `c3 a9`.
- Every digest is lowercase hex. `crc32` always has eight digits, with leading
  zeros, so `crc32("")` is `00000000`.
- A file is read 64 KiB at a time, so memory use stays small whatever the
  file's size. An empty file has the digest of nothing.
- `hmac_sha256` takes the key and the text as UTF-8 bytes, and a key of any
  length.
- `same` is false at once for strings of different lengths. For equal lengths
  it compares every byte before it answers.

## Security

None of these is for storing passwords. They are fast on purpose, which is
the wrong property for a password, where a guess must be slow and salted. A
password needs a function built to be slow, such as Argon2, bcrypt, or scrypt,
and `std.hash` has none.

MD5 and SHA-1 are broken for security. Someone can build two different texts
with the same digest, so they cannot sign anything, check a certificate, or
stand in for a digest an attacker might forge. They are still fine for
checksums that guard against accidental damage, such as a download corrupted
on the way. For new work, prefer `sha256`, or `hmac_sha256` when a key is
involved.

CRC-32 finds accidental damage quickly, and nothing more: anyone can change a
text so that it has any CRC-32 they choose.

## How it works

The digests, reading files, and comparing bytes are native
([std/rs/hash.rs](../../std/rs/hash.rs)), using the RustCrypto crates `sha2`,
`sha1`, and `md-5`, the `hmac` crate, and `crc32fast`. The comparison is
native because Marslang has no bitwise operators, and an interpreted loop that
stopped at the first difference would show where two digests part.

The choice of algorithm, the error families, and the messages are Marslang,
in [std/hash.mars](../../std/hash.mars).
