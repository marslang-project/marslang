# std.file.path

Working with path text: joining, splitting, and changing names and extensions.
Nothing here reads the disk, except `absolute`, which starts from the working
directory.

```mars
takepkg std.file.path;

func m{
    report = path.join("reports", "june.csv");
    out(report);                                // reports\june.csv on Windows, reports/june.csv elsewhere
    out(path.stem(report), path.extension(report));   // june csv
    out(path.with_extension(report, "json"));   // reports\june.json
}
```

| Function | Result | For `docs/api/std.md` |
| --- | --- | --- |
| `join(a, b)` | `b` joined onto `a` with the platform's separator | |
| `parent(p)` | The directory part | `docs/api` |
| `name(p)` | The last part | `std.md` |
| `stem(p)` | The name without its extension | `std` |
| `extension(p)` | The extension, without its dot | `md` |
| `with_extension(p, ext)` | The path with another extension | `docs/api/std.txt` for `"txt"` |
| `absolute(p)` | The full path, from the working directory | |
| `is_absolute(p)` | Whether it starts from a root or a drive | `false` |
| `parts(p)` | Every part, as an array | `["docs", "api", "std.md"]` |
| `normalize(p)` | `.` removed and `..` taken back | `a/./b/../c` is `a/c` |
| `relative(p, base)` | The path that leads from `base` to `p` | `api/std.md` from `docs` |

Only the last extension counts: `archive.tar.gz` has the stem `archive.tar` and
the extension `gz`. A name that starts with a dot, such as `.bashrc`, is all
stem and has no extension. `with_extension` accepts `"md"` or `".md"`, and
`""` removes the extension.

`parent` of a bare name such as `std.md` is `""`, meaning the working directory.
`join` returns `b` unchanged when `b` is itself absolute, as Python's
`os.path.join` does.

On Windows both `/` and `\` separate parts, and a drive is the first part,
`C:\`. Elsewhere only `/` separates, and `\` is an ordinary character in a
name. The separator `join` uses is `os.SEP` in [std.os](std-os.md).

`absolute` does not resolve links and does not require the path to exist.

`normalize` works on the text alone: `..` takes back the name before it, a `..`
at the start of a relative path stays, and one at a root stays at the root. A
path that normalizes to nothing is `"."`. Because it does not look at the disk,
it does not resolve links; `file.same` does.

`relative` gives the way from one directory to a path: from `docs/api` to
`docs/guide` is `../guide`. When one path is absolute and the other is not,
both are taken from the working directory. Paths on different drives, or a base
that climbs above where it starts with `..`, raise `RangeError`.

The wrapper is written in Marslang ([std/file/path.mars](../../std/file/path.mars))
over the native package [std/rs/file.rs](../../std/rs/file.rs).
