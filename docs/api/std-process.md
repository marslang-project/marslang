# std.process

Run other programs and collect what they print. There is no shell: you give
the program and its arguments as an array, and each argument reaches the
program exactly as written.

```mars
takepkg std.process;

func m{
    r = process.run(arr("cmd", "/c", "echo", "hello"));   // on Linux: arr("echo", "hello")
    out(r.code, r.ok);                  // 0 true
    out(r.output.strip());              // hello

    run {
        process.check(arr("cmd", "/c", "exit", "3"));
    } handle(process.ProcessError e){
        out(e.code);                    // 3
    }
}
```

| Function | Result |
| --- | --- |
| `run(args)` | Run the program, wait for it, and give a `Result` |
| `run_in(args, folder)` | The same, with `folder` as its working folder |
| `run_with(args, input)` | The same, with the string `input` as its standard input |
| `check(args)` | Like `run`, but raises `ProcessError` unless the code is 0 |
| `which(name)` | The full path of the program `name` runs, or `null` |
| `set_timeout(seconds)` | Kill programs that run longer; `null` removes the limit |
| `timeout()` | The limit in seconds, or `null` |

`args` is an array of strings: the program first, then its arguments. A
`Result` has these fields:

| Field | Meaning |
| --- | --- |
| `code` | The exit code, an int; `null` if a signal killed the program |
| `ok` | Whether `code` is 0 |
| `output` | What the program wrote to its standard output, as text |
| `errors` | What it wrote to its standard error, as text |

Output that is not UTF-8 is turned into text with each bad byte replaced by
U+FFFD. A program that fails is still a `Result`: `run` raises nothing for a
non-zero code, so look at `ok` or `code`, or use `check`.

## There is no shell

Many languages run `"ls " + name` through a shell, so a name such as
`x; rm -rf ~` runs a second command. `process.run` never starts a shell. The
array is the program and its arguments, and the program gets each element as one
argument, whatever characters it holds:

```mars
takepkg std.process;

func m{
    // echo gets ONE argument, "a; rm x". Nothing is removed.
    r = process.run(arr("cmd", "/c", "echo", "a; rm x"));
    out(r.output.strip());              // "a; rm x" (cmd's echo shows the quotes; on Linux: a; rm x)
}
```

Nothing is expanded either: `*` is not a list of files, `$HOME` is not a
folder, and quotes around an argument are part of it. When you do want shell
features, such as pipes or redirection, start a shell yourself and give it the
command as one argument, knowing that the text is then a shell command:
`process.run(arr("sh", "-c", "ls | wc -l"))`, or on Windows
`arr("cmd", "/c", "dir | find /c /v \"\"")`. Never build that text from input you
do not trust.

## Input, folders, and the environment

```mars
takepkg std.process;

func m{
    r = process.run_with(arr("sort"), "pear\napple\n");
    out(r.output.strip().lines());      // ["apple", "pear"]
    // process.run_in(arr("git", "status", "--short"), "/home/ada/project")
}
```

- **Input.** `run_with` writes the string to the program's standard input and
  closes it. With `run`, `run_in`, and `check` the program reads an empty
  input, so a program that waits for a line does not hang.
- **Folder.** `run_in` starts the program in `folder`; a folder that does not
  exist raises `file.NotFoundError` and a file raises `file.NotDirectoryError`. A program
  given as a relative path with a folder part, such as `./tool`, is taken
  from this program's own folder, not from `folder`, on every system.
- **Environment.** The program gets this program's environment, unchanged.
  There is no way yet to add or remove variables for one program.

## Finding programs

A program name without a folder part is looked up in the PATH folders, in
order, as the system does. On Windows a name is tried with each extension in
`PATHEXT`, so `npm` runs `npm.cmd`, and `run` and `which` agree. A name with a
folder part is that path, relative to the current folder. `which` gives the
full path of the file that would run, or `null`:

```mars
takepkg std.process;

func m{
    out(process.which("no-such-program-anywhere"));   // null
    out(process.which("cmd") != null);                // true on Windows
}
```

## Errors

| Error | When |
| --- | --- |
| `process.NotFoundError` | The program is not installed; the message names it |
| `process.TimeoutError` | The program ran longer than `set_timeout` allows, and was killed |
| `process.ProcessError` | `check` found a non-zero code; the program cannot be run (not executable, a directory); it wrote more than 100 MB to a stream |
| `RangeError` | `args` is empty or starts with an empty string; a timeout that is not positive |
| `TypeError` | `args` is not an array of strings |
| `file.NotFoundError` | The folder given to `run_in` does not exist |

`NotFoundError` and `TimeoutError` inherit from `ProcessError`, so
`handle(process.ProcessError e)` catches all three. After a failed `check`,
`e.code`, `e.output`, and `e.errors` hold the program's outcome, and the
message holds them too, shortened to a screenful:

```text
error: ProcessError: the program "git" ended with code 128
errors:
fatal: not a git repository (or any of the parent directories): .git
```

## Time and size limits

- **Time.** There is no limit unless you set one: `process.set_timeout(10)`
  kills every program started afterwards that runs longer than ten seconds, and
  raises `TimeoutError`. Only the program is killed, not programs it started.
  `process.set_timeout(null)` removes the limit.
- **Size.** Each of the two streams, output and errors, may hold 100 MB. A
  program that writes more is killed and `ProcessError` is raised. For larger
  output, have the program write to a file.

## Tasks

A program runs on a worker thread, so inside a [task](async.md) only that task
waits and the others keep running. Three programs that take a second each,
started in three tasks, take a second together (here `ping` pauses for one):

```mars
takepkg std.process;
takepkg std.async;

async func pause(string n) => process.run(arr("ping", "-n", "2", "127.0.0.1")).ok;   // Linux: "-c", "2"

func m{
    out(await async.all(arr(pause("a"), pause("b"), pause("c"))));   // [true, true, true]
}
```

Cancelling a task kills the program it is waiting for, within a few
milliseconds, and the task raises `CancelledError` at once. The same happens
to a program a task is waiting for when the program ends. Outside any task,
`run` waits and blocks, as in a program that does not use tasks.

## How it works

Starting the program, writing its input, reading its two streams, the clock,
and the PATH lookup are native ([std/rs/process.rs](../../std/rs/process.rs)),
by Rust's `std::process::Command`, which starts the program directly without
a shell. The worker polls for the end of the program every millisecond at
first and then every ten, so it can notice a cancellation or the time limit.
`Result`, the errors, and `check` are Marslang
([std/process.mars](../../std/process.mars)).

## Limits

- No editing of the environment, no changing of the program's standard error
  into its output, and no streaming: the output arrives as a whole when the
  program ends.
- A program that starts others which keep its output open delays `run` until
  they close it or the time limit passes.
- Killing on a timeout or cancellation ends the program only, not the programs
  it started.
