# std.file

Reading, writing, checking, listing, copying, moving, and removing files and
directories. Text is UTF-8. Working with path text is
[std.file.path](std-file-path.md), CSV tables are [std.file.csv](std-file-csv.md),
and JSON files are [`json.load` and `json.save`](std-json.md#files).

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
| `read_lines(path)` | The lines, as an array, without their line endings |
| `each_line(path, f)` | Calls `f(line)` for each line, reading as it goes |

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

## Directories

| Function | Result |
| --- | --- |
| `list(dir)` | The names directly inside, sorted |
| `walk(dir)` | The path of every file below, at any depth, sorted |
| `make_dir(path)` | Creates the directory and any missing parents |

`list` gives names, and `walk` gives paths that start with `dir`, so they can be
passed straight to `read`. `walk` lists files only, and does not follow a link
to a directory, so a link back up the tree cannot loop. `make_dir` does nothing
when the directory is already there.

## Copying, moving, and removing

| Function | Result |
| --- | --- |
| `copy(from, to)` | Copies a file, replacing one at `to` |
| `move(from, to)` | Moves or renames a file or a directory |
| `remove(path)` | Removes a file |
| `remove_dir(path)` | Removes an empty directory |
| `remove_all(path)` | Removes a directory and everything in it |

`to` is the full path of the result, not the directory to put it in. `move`
works across drives for files, by copying and then removing.

`remove_all` cannot be undone, so it refuses the targets nobody means: the root
of a drive or file system, the home directory, and the working directory or any
directory that contains it. Each raises `PermissionError`.

## Places

| Function | Result |
| --- | --- |
| `here(name)` | `name` beside the running program's own file |
| `temp_dir()` | The system's directory for temporary files |

A relative path such as `"data.json"` is read from the directory the program
was started in. That changes when someone runs `marslang tools/report.mars`
from elsewhere, and the program stops finding its files. `file.here("data.json")`
is the file next to `report.mars` wherever it is started from.

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
settings = "{}";
run{
    settings = file.read("settings.json");
} handle(file.NotFoundError e){
    out("no settings.json yet, so the defaults are used");
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
