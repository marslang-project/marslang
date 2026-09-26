# rs-0.15.0 — Files, dates, and every Linux

Programs can now work with files and folders through [std.file](../api/std-file.md),
read and write configuration in CSV, `.env`, INI, and TOML, and handle the
calendar and the clock with three new kinds: [`date`, `datetime`, and
`duration`](../api/dates.md). The Linux releases are now static binaries that
run on every distribution.

## std.file

```mars
takepkg std.file;
takepkg std.json;

func m{
    settings = json.load(file.here("settings.json"));
    settings.set("theme", "dark");
    json.save(file.here("settings.json"), settings);   // all or nothing
}
```

Reading, writing, checking, listing, copying, moving, and removing files and
directories, in 28 functions. `read_or` covers a file that may not exist yet,
`each_line` streams a file larger than memory, `matching` finds files by
pattern (`"**/*.mars"`), and `with_temp_dir` gives a function a scratch
directory that is removed afterwards, even when the function raises.
`here(name)` finds a file beside the running program, however it was started.

Writing is all or nothing: the text goes to a temporary file that then replaces
the old one, so a crash never leaves half a file. Every failure is a family you
can catch on its own, such as `file.NotFoundError`, and every message names the
path. `remove_all` refuses a drive root, the home directory, and the working
directory.

## Paths and file formats

| Package | For |
| --- | --- |
| [std.file.path](../api/std-file-path.md) | Joining, splitting, `normalize`, and `relative` |
| [std.file.csv](../api/std-file-csv.md) | CSV as rows or as records keyed by the header, with any separator |
| [std.file.env](../api/std-file-env.md) | `.env` files of `KEY=value` lines |
| [std.file.ini](../api/std-file-ini.md) | INI files of `[sections]` |
| [std.file.toml](../api/std-file-toml.md) | TOML 1.1, as Cargo and Python projects use it |
| [std.json](../api/std-json.md) | Gains `load` and `save` for JSON files |

A mistake in a file names it and the line: `people.csv: CSV line 4: 3 fields,
but the header has 2`.

## date, datetime, and duration

```mars
takepkg std.time;

func m{
    left = date(2026, 12, 31) - time.today();
    out("days left:", left.days());

    meeting = datetime(2026, 9, 24, 14, 30, 0, "Asia/Shanghai");
    out(meeting.in_zone("America/New_York"));   // 2026-09-24T02:30:00-04:00[America/New_York]
    out(meeting + time.hours(1) + time.minutes(30));
}
```

They are values, like numbers: equal days are `==`, `<` and sorting order them,
and they work as map keys. A `datetime` is a moment seen in a time zone, with
IANA names and daylight saving; two are equal when they are the same moment. A
`duration` is an exact length, so `ny + time.days(1)` is exactly 24 hours later,
while `ny.add_days(1)` keeps the time of day across a clock change.

`std.time` gains `today()`, `now_datetime()`, and `days()`, `hours()`,
`minutes()`, `seconds()`, and `milliseconds()`, and `sleep` takes a duration.
TOML reads and writes real dates; JSON writes them as ISO 8601 strings.

## Linux, everywhere

The Linux releases needed glibc 2.34 or newer, so they did not start on
Debian 11, Ubuntu 20.04, RHEL 8, or Amazon Linux 2, and never on Alpine. They
are now static binaries, for x86_64 and ARM64, and run on all of them. The
ARM64 build is now tested on ARM hardware, where before it was cross-built and
never run. A built-in copy of the time zone database is used where the system
has none, as in minimal containers.

`install.sh` picks the static archive, and still installs older releases,
which only have glibc ones.

## Fixes

- `bool` is accepted as a type annotation: `func check(bool ok)` used to raise
  `unknown type bool`.

## Validation

`cargo test` passes 151 tests (13 unit, 137 execution, 1 memory) on Windows,
macOS, and Linux on x86_64 and ARM64. Every push also runs the static Linux
binary inside Alpine, Debian 11, Ubuntu 20.04, Rocky Linux 8, and Amazon Linux 2
with `tests/platform/check.mars`, and checks that it is as fast as a glibc
build. Every example on the new documentation pages was run against this build.
