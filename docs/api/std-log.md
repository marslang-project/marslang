# std.log

Leveled logging for programs and scripts: debug, info, warn, and error lines,
with a timestamp, written to standard error or to a file, and named loggers
for different parts of a program.

```mars
takepkg std.log;

func m{
    log.info("starting");                        // 2026-10-13 14:05:09 INFO  starting
    log.debug("hidden at the default level");    // not written
    log.set_level("debug");
    log.debug("now written");                    // 2026-10-13 14:05:09 DEBUG now written

    db = log.Logger("db");
    db.warn("slow query");                       // 2026-10-13 14:05:09 WARN  [db] slow query
}
```

Log lines go to standard error, not standard output, so they never mix with
what the program prints. Run as `marslang prog.mars > out.txt`, the file holds
only the program's own output and the logs still show on the screen.

| Function | Result |
| --- | --- |
| `debug(message)` | Writes message at the debug level |
| `info(message)` | Writes message at the info level |
| `warn(message)` | Writes message at the warn level |
| `error(message)` | Writes message at the error level |
| `set_level(name)` | Sets the lowest level written: `"debug"`, `"info"`, `"warn"`, `"error"`, or `"off"`; returns `null` |
| `level()` | The current level name |
| `to_file(path)` | Appends lines to the file at path instead of standard error; returns `null` |
| `to_screen()` | Writes lines to standard error again; returns `null` |
| `Logger(name)` | A logger whose lines carry `[name]` after the level |

Each message is converted to text with `string`, so it can be any value.

## Logger

| Method | Result |
| --- | --- |
| `debug(message)`, `info(message)`, `warn(message)`, `error(message)` | As the functions above, with the logger's name |
| `set_level(name)` | Sets this logger's own level, which overrides the shared one; returns `null` |
| `level()` | This logger's level: its own if set, otherwise the shared one |

```mars
takepkg std.log;

func m{
    log.set_level("warn");
    db = log.Logger("db");
    out(db.level());                  // warn: it follows the shared level
    db.info("not written");           // below warn, so nothing
    db.set_level("info");
    out(db.level());                  // info: now its own
    db.info("written");               // 2026-10-13 14:05:09 INFO  [db] written
    out(log.level());                 // warn: the shared level is unchanged
}
```

## Levels and output

The levels in order are `debug`, `info`, `warn`, and `error`. A message is
written when its level is at or above the current one, so at `"warn"` the
`warn` and `error` lines show and the others do not. `"off"` is above every
level and writes nothing. The default is `"info"`, so `debug` lines are hidden
until `log.set_level("debug")`.

Each line has the local time, the level, and the message:

```text
2026-10-13 14:05:09 INFO  loaded 42 rows
2026-10-13 14:05:09 WARN  [db] slow query
```

The level name is padded to five characters so the messages line up. The time
is the local time of the machine, to the second, in the format
`%Y-%m-%d %H:%M:%S`.

```mars
takepkg std.log;
takepkg std.file;

func m{
    log.to_file("app.log");
    log.info("written to the file");
    log.to_screen();
    log.info("back on standard error");
}
```

`to_file` appends, so a log keeps growing across runs. Use `file.write`
from [std.file](std-file.md) to start a file empty.

## Edge cases

- A new logger follows the shared level until its own `set_level` is called.
  Changing the shared level afterwards then changes it again for that logger.
- `set_level("off")` on a logger silences only that logger.
- Messages are not split or wrapped: a message with a newline in it takes two
  lines in the output.
- An array as a message prints as `string` makes it, for example `[1, 2]`.

## Errors

`set_level` and a logger's `set_level` raise `RangeError` for any name other
than the five above, including a value that is not a string, and the message
names the choices:

```text
log.set_level needs "debug", "info", "warn", "error", or "off", got "loud"
```

Writing to a file set by `to_file` uses [std.file](std-file.md) and so can
raise its `FileError`s, such as `PermissionError`, from the log call itself.

## How it works

The native part, `std/rs/log.rs`, is one function: writing text and a newline
to standard error and flushing it. Marslang has no way to reach standard error
on its own. Everything else is Marslang in `std/log.mars`: the level ranks,
the timestamp from [std.time](std-time.md), the choice between standard error
and a file, and the `Logger` family.
