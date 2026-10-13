# std.terminal

Coloured and styled text, the terminal's size, clearing the screen, and
progress bars, for command-line tools and games. Styles return the text wrapped
in ANSI escape codes, so they fit inside [f-strings](strings.md#f-strings), and
they return it plain when colours are off, so the same program is clean in a
log file.

```mars
takepkg std.terminal;

func m{
    out(terminal.bold(terminal.red("error:")) + " the file is missing");
    out(f"{terminal.green('ok')} {terminal.dim('(12 ms)')}");
    out(terminal.progress(0.5, 20));
    out(terminal.columns() > 0);
}
```

On a terminal, the first line is a bold red `error:` followed by plain text.
Piped into a file, the output is

```text
error: the file is missing
ok (12 ms)
[##########----------]  50%
true
```

## Styles and colours

| Function | Result |
| --- | --- |
| `bold(text)`, `dim(text)`, `italic(text)`, `underline(text)` | The text in that style |
| `red`, `green`, `yellow`, `blue`, `magenta`, `cyan`, `gray` (`text`) | The text in that colour |
| `color(text, name)` | The text in a colour chosen by name: one of the seven above |
| `rgb(text, r, g, b)` | The text in a 24-bit colour; each of `r`, `g`, `b` is 0 to 255 |
| `strip(text)` | The text without its ANSI escape sequences |

Each takes any value and converts it with `string()`. With colours on, the
result is an SGR escape sequence, the text, and a reset (`ESC[0m`); `red("x")`
is `"\u001b[31mx\u001b[0m"`. Styles nest: `bold(red("x"))` is bold and red, and
in `red("a" + bold("b") + "c")` the red starts again after the bold part, so
`"c"` is red too. `gray` is the bright form of black. Italics and 24-bit colour are not shown by a
few older terminals, which ignore them or use the nearest colour they have.

An unknown name given to `color` raises `RangeError` listing the known names,
and so does a value outside 0 to 255 given to `rgb`. Both are checked whether
or not colours are on, so a mistake does not wait for a colour terminal to show
up.

`strip` removes every CSI sequence, `ESC[`, then parameter bytes, then a final
byte from `@` to `~`: styles, and cursor and screen codes too. Use it for a
log file, or to measure text. `strip(bold(red(text)))` is always `text`.

## When colours are on

`terminal.colors()` says whether the styles above add codes or return plain
text. It is decided once, the first time it is needed:

1. If `FORCE_COLOR` is set and not empty, colours are **on**, even when output
   is a file or a pipe.
2. Otherwise, if `NO_COLOR` is set and not empty, they are **off**
   (see [no-color.org](https://no-color.org)).
3. Otherwise they are on when standard output is a terminal and, on Windows,
   the console accepts ANSI codes.

`terminal.set_colors(on)` overrides the answer for the rest of the run, whatever
the terminal and the environment say: connect it to a `--color` or `--no-color`
flag, or use it in a test to see the exact codes. `colors()` then returns what
was set.

```mars
takepkg std.terminal;

func m{
    terminal.set_colors(true);
    out(terminal.strip(terminal.bold("Done")));    // Done
    terminal.set_colors(false);
    out(terminal.bold("Done"));                    // Done
}
```

Both lines print plain `Done`: the first has its codes stripped, the second
never had them.

## The terminal

| Function | Result |
| --- | --- |
| `is_terminal()` | Whether standard output is a terminal, not a file or a pipe |
| `columns()` | The terminal's width in columns |
| `rows()` | The terminal's height in rows |
| `clear()` | Clears the screen and moves to its top left corner; returns `null` |

When standard output is not a terminal, or its size is unknown, `columns()`
and `rows()` read the `COLUMNS` and `LINES` environment variables (whole
numbers above 0), and otherwise are 80 and 24. They always return a positive
`int`, and are asked again on every call, so they follow the window as it is
resized.

`clear()` writes `ESC[2J ESC[H`, and only when colours are on: in a file or a
pipe the codes would be clutter, so it prints nothing.

## Progress bars

`progress(fraction, width)` returns a bar as text: `fraction` from 0.0 to 1.0,
`width` cells inside the brackets, then the percentage right-aligned in four
characters.

```mars
takepkg std.terminal;

func m{
    out(terminal.progress(0, 10));
    out(terminal.progress(0.5, 20));
    out(terminal.progress(1, 10));
}
```

prints

```text
[----------]   0%
[##########----------]  50%
[##########] 100%
```

The bar and the percentage round down, so neither shows complete before the
work is: `progress(0.99, 10)` is nine cells and `99%`. A fraction outside 0 to 1
and a width below 1 raise `RangeError`. The bar has no colour of its own; wrap
it in `green(...)` if you like.

To show progress on one line, end it without a newline and start the next
update with a carriage return, `\r`, which moves back to the start of the line.
Print a newline once at the end:

```mars
takepkg std.terminal;

func m{
    total = 40;
    for (done = 0, done <= total, done += 5){
        slout("\r" + terminal.progress(done / total, 30));
    }
    out("");
}
```

In a terminal the bar fills in place; a bar is always the same length for a
given `width`, so each update covers the last completely. When output is a file,
each update is simply written after the last, so check `terminal.is_terminal()`
and print only the final bar there.

## Windows

Colours work in Windows Terminal, in PowerShell, and in the consoles of Windows
10 and later. Windows turns ANSI codes off in a console by default; the first
time `colors()` is decided, `std.terminal` turns them on for the program's
console (`ENABLE_VIRTUAL_TERMINAL_PROCESSING`), once, and if Windows refuses,
as an old console does, colours stay off. Terminals that are not Windows
consoles but take ANSI codes themselves, such as Git Bash's, are on already.
The size is that of the visible window, not of its scrollback.

## Errors

An unknown name for `color`, an `r`, `g`, or `b` outside 0 to 255, a `progress`
fraction outside 0 to 1, and a `progress` width below 1 raise `RangeError`.

## How it works

Only what the operating system alone can say is native
([std/rs/terminal.rs](../../std/rs/terminal.rs)): whether output is a terminal,
its size (the `terminal_size` crate on Linux and macOS, the console's own call
on Windows), and turning ANSI on in a Windows console. Everything else is
Marslang, in [std/terminal.mars](../../std/terminal.mars): building the escape
sequences, deciding when colours are on, stripping them (with
[std.regex](std-regex.md)), clearing, and drawing progress bars.
[std.format](std-format.md) knows about the sequences too, so `format.width`,
`left`, and `table` line up coloured text.
