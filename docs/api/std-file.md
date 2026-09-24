# std.file

Reading, writing, checking, listing, copying, moving, and removing files and
directories. Text is UTF-8. Working with path text is
[std.file.path](std-file-path.md), CSV tables are [std.file.csv](std-file-csv.md),
and JSON files are [`json.load` and `json.save`](std-json.md#files).
Configuration files are [std.file.env](std-file-env.md),
[std.file.ini](std-file-ini.md), and [std.file.toml](std-file-toml.md).

```mars
takepkg std.file;

func m{
    name = file.here("notes.txt");
    file.write(name, "first line\n");
    file.append(name, "second line\n");
    for (line, file.read_lines(name)){ out(line); }
}
```

## Reading

| Function | Result |
| --- | --- |
| `read(path)` | The whole file as a string |
| `read_or(path, fallback)` | The whole file, or `fallback` when there is no such file |
| `read_lines(path)` | The lines, as an array, without their line endings |
| `each_line(path, f)` | Calls `f(line)` for each line, reading as it goes |

`read_or` covers the most common case of a missing file in one line, such as
settings that have not been saved yet: `file.read_or("settings.json", "{}")`.
Only a missing file gives the fallback; one that exists but cannot be read
still raises.

`read` keeps line endings exactly as the file has them. `read_lines` and
`each_line` remove `\n` and `\r\n`, and a line ending at the very end adds no
empty line.

`each_line` holds about a thousand lines at a time, so it processes a file
larger than memory:

```mars
errors = 0;
func check(string line){ if (line.contains("ERROR")){ errors = errors + 1; } }
file.each_line("server.log", check);
out(errors);
```

A file that is not UTF-8 text raises `FileError` and names the first bad byte:

```text
FileError: photo.png: the file is not UTF-8 text (byte 0x89 at offset 0); std.file reads text files only
```

## Writing

| Function | Result |
| --- | --- |
| `write(path, text)` | Replaces the contents, creating the file if needed |
| `append(path, text)` | Adds to the end, creating the file if needed |
| `create(path, text)` | Writes a new file; `ExistsError` if one is already there |
| `write_lines(path, lines)` | Writes an array of strings, each followed by `\n` |

`write` and `write_lines` are all or nothing. The text goes to a temporary file
beside the target, which is flushed to disk and then renamed over the old file,
so a crash, a full disk, or a power cut leaves either the old contents or the
new, never half of a file. `append` adds in place.

The text is written exactly as given on every platform: `\n` stays `\n` on
Windows, so a file is the same bytes wherever it was written.

The directory must already exist. Writing into one that does not raises
`NotFoundError` and says so, rather than creating a directory from a typo:

```text
NotFoundError: reports/june.txt: the directory reports does not exist; file.make_dir creates it
```

## Checking

| Function | Result |
| --- | --- |
| `exists(path)` | Whether a file or a directory is there |
| `is_file(path)`, `is_dir(path)` | Whether a file, or a directory, is there |
| `size(path)` | The size in bytes: an `int`, or a `longint` beyond 2 GB |
| `modified(path)` | When it last changed, in seconds since 1970, as `time.now()` gives |
| `info(path)` | All of these facts at once, as a map |
| `same(a, b)` | Whether two paths name the same file or directory |

`info` gives `kind` (`"file"`, `"directory"`, or `"other"`), `link` (whether the
path itself is a link), `size` (`null` for a directory), `modified`, `created`
(`null` where the system does not record it), and `readonly`. A link is
followed, so `kind` describes what it points to.

`same` resolves links, `.`, and `..` before comparing, so
`file.same("a.txt", "./sub/../a.txt")` is `true` when `sub` is a directory beside
`a.txt`. Both paths must exist; on Linux and macOS that includes every directory
named on the way, even one that `..` leaves again.

## Directories

| Function | Result |
| --- | --- |
| `list(dir)` | The names directly inside, sorted |
| `walk(dir)` | The path of every file below, at any depth, sorted |
| `matching(dir, pattern)` | The paths below `dir` that match a pattern, sorted |
| `make_dir(path)` | Creates the directory and any missing parents |

`list` gives names, and `walk` gives paths that start with `dir`, so they can be
passed straight to `read`. `walk` lists files only, and does not follow a link
to a directory, so a link back up the tree cannot loop. `make_dir` does nothing
when the directory is already there.

### Patterns

```mars
file.matching("logs", "*.txt");       // text files directly in logs
file.matching("src", "**/*.mars");    // .mars files at any depth
file.matching(".", "report-202?.csv");
```

| In a pattern | Matches |
| --- | --- |
| `*` | Any run of characters within one name |
| `?` | One character |
| `[abc]`, `[a-z]`, `[!abc]` | One character of the set, or not of it |
| `**` | Any number of directories, none included |

Patterns use `/` between names on every platform; `\` works too on Windows.
Files and directories both match. A name starting with a dot is matched only by
a pattern part that starts with one, as in a shell, so `**/*.mars` does not
reach into `.git`. Case is ignored on Windows, where file names ignore it too.
A pattern is relative to `dir` and cannot leave it with `..`.

## Copying, moving, and removing

| Function | Result |
| --- | --- |
| `copy(from, to)` | Copies a file, replacing one at `to` |
| `copy_all(from, to)` | Copies a directory and everything in it to a new directory `to` |
| `move(from, to)` | Moves or renames a file or a directory |
| `remove(path)` | Removes a file |
| `remove_dir(path)` | Removes an empty directory |
| `remove_all(path)` | Removes a directory and everything in it |

`to` is the full path of the result, not the directory to put it in. `move`
works across drives for files, by copying and then removing.

`copy_all` makes `to`, which must not exist yet, so it never merges into an
existing directory. If it fails partway, what it made is removed again. It
refuses to copy a directory into itself, and a link to a directory inside,
since following one could copy far more than was asked.

`remove_all` cannot be undone, so it refuses the targets nobody means: the root
of a drive or file system, the home directory, and the working directory or any
directory that contains it. Each raises `PermissionError`.

## Places

| Function | Result |
| --- | --- |
| `here(name)` | `name` beside the running program's own file |
| `temp_dir()` | The system's directory for temporary files |
| `with_temp_dir(f)` | Calls `f` with a new, empty directory, then removes it |

A relative path such as `"data.json"` is read from the directory the program
was started in. That changes when someone runs `marslang tools/report.mars`
from elsewhere, and the program stops finding its files. `file.here("data.json")`
is the file next to `report.mars` wherever it is started from.

`with_temp_dir` is for scratch work and tests: the directory is removed with
everything in it after `f` returns, and also when `f` raises, and it returns
what `f` returns.

```mars
takepkg std.file;
takepkg std.file.path;

func m{
    func work(string dir){
        file.write(path.join(dir, "draft.txt"), "...");
        ret file.list(dir);
    }
    out(file.with_temp_dir(work));   // ["draft.txt"]
}
```

## Errors

Every failure is a family under `file.FileError`, and every message starts with
the path:

| Family | When |
| --- | --- |
| `file.FileError` | Any failure; catches all of those below |
| `file.NotFoundError` | Nothing is there, or a directory on the way is missing |
| `file.ExistsError` | `create` or `make_dir` found something already there |
| `file.PermissionError` | The system refused, or `remove_all` refused |
| `file.IsDirectoryError` | An operation on files was given a directory |
| `file.NotDirectoryError` | An operation on directories was given a file |

```mars
run{
    file.remove(file.here("old.log"));
} handle(file.NotFoundError e){
    out("nothing to clean up");
}
```

Removing a directory that is not empty raises `FileError` and points to
`remove_all`.

## Not yet

Binary files need a bytes type the language does not have. Open file handles
(`open`, `read_line`, `seek`, `close`), `*.txt` patterns, watching for changes,
and permissions are later work.

The wrapper is written in Marslang ([std/file/init.mars](../../std/file/init.mars))
over the native package [std/rs/file.rs](../../std/rs/file.rs).
