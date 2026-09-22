# std.cli

`std.cli` gives a program its command line: the words after its file, a parser
for `--flags`, `--options`, and positional arguments, and a way to stop with an
exit status.

```mars
takepkg std.cli;

func m{
    p = cli.parser("greet", "Say hello to someone.");
    p.flag("loud", "Shout the greeting");
    p.option("name", "Who to greet", "world");
    p.positional("file", "A file to read");
    opts = p.parse();

    greeting = "hello, " + opts.get("name");
    if (opts.get("loud")){ greeting = greeting.upper(); }
    out(greeting, "from", opts.get("file"));
}
```

```sh
marslang greet.mars notes.txt --name Ada --loud
```

```text
HELLO, ADA from notes.txt
```

Everything after the program's file is given to the program, with
`marslang greet.mars ...` and with `marslang run greet.mars ...` alike.

## Functions

| Function | Result |
| --- | --- |
| `args()` | The words after the program's file, as an array of strings |
| `program()` | The path of the program's file, as it was given |
| `exit(code)` | Stops the program with that exit status; `0` means success |

`exit` is not an error: no `handle` catches it, but `then` blocks still run on
the way out, so cleanup still happens. A program that reaches the end of `m`
exits with status `0`, and one stopped by an uncaught error with `1`.

## parser

`cli.parser(name, description)` makes a parser; `name` and `description` appear
in `--help`. Declare what the program accepts, then call `parse()`:

| Method | Declares |
| --- | --- |
| `flag(name, help)` | `--name`, which sets `name` to `true`; `false` when absent |
| `option(name, help, default)` | `--name VALUE` or `--name=VALUE`; `default` when absent |
| `positional(name, help)` | A required word that is not an option, in declaration order |
| `parse()` | Reads `args()` and returns a map from each name to its value |
| `parse_args(words)` | The same, for an array of words you pass |
| `usage()` | The `--help` text |

Each declaring method returns the parser, so they can be chained. Option values
and positionals are strings; convert them with `int(...)` or `float(...)`. A
lone `--` makes every later word positional.

`--help` or `-h` prints the usage and exits with status `0`:

```text
Usage: greet [--loud] [--name NAME] FILE

Say hello to someone.

Arguments:
  FILE         A file to read

Options:
  --loud       Shout the greeting
  --name NAME  Who to greet (default: world)
  -h, --help   Show this help and exit
```

Words the parser cannot accept raise `cli.UsageError`: an unknown option, an
option without its value, a value given to a flag, a missing positional, or one
more positional than declared. Uncaught, it ends the program with a message
such as `error: UsageError: unknown option --nope; run with --help to see the
options`. Catch it with `handle(cli.UsageError e)` to report it your own way.

## Reading input

`in()` reads all of standard input as one string, and `inln()` reads it a line
at a time; see [input and output](builtins.md#input-and-output).
