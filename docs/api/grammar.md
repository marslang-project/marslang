# Grammar

The syntax the interpreter accepts, written as a grammar. Where the
[language reference](language.md) explains what programs mean, this page says
exactly what may be written. `src/parser.rs` and `src/expression.rs` are the
implementation; a program they reject that this page allows is a bug in one of
them.

## Notation

`A B` is A followed by B, `A | B` either, `[ A ]` optional, `{ A }` zero or more
times, `( ... )` grouping, and quoted text literal. `NAME`, `NUMBER`, and
`STRING` are the tokens under [lexical structure](#lexical-structure).

## Programs

```text
program      = { item } ;
item         = import | decorated | family | function | declaration | statement ;
import       = "takepkg" package [ "=" NAME ] ";" ;
package      = { "." } NAME { "." NAME } ;
decorated    = decorator { decorator } ( function | family ) ;
decorator    = "@" NAME "." NAME [ "(" expression ")" ] ;
family       = "family" NAME [ "(" parent ")" ] "{" { [ { decorator } ] function } "}" ;
parent       = NAME | NAME "." NAME ;
function     = "func" NAME [ "(" [ params ] ")" ] ( block | "=>" expression ";" ) ;
params       = param { "," param } ;
param        = type NAME ;
```

A program runs from `func m`. Top-level declarations and statements run first,
in order, after the packages they import. Several decorators may share a line.
`takepkg .name;` and `takepkg ..name;` are relative imports; `= *` is reserved.

## Statements

```text
block        = "{" { statement } "}" ;
statement    = declaration | assignment | update | expression ";"
             | "ret" expression ";" | "break" ";" | "continue" ";"
             | if | while | repeat | for | run | function ;
declaration  = [ "fixed" ] [ "hot" | "cold" ] NAME [ "(" type ")" ] "=" expression ";" ;
assignment   = target "=" expression ";" ;
target       = NAME | postfix "." NAME ;
update       = NAME ( "+=" | "-=" ) expression ";" | target ( "++" | "--" ) ";" ;
if           = "if" "(" expression ")" block { "elif" "(" expression ")" block } [ "else" block ] ;
while        = "while" "(" expression ")" block ;
repeat       = "repeat" expression block ;
for          = "for" "(" NAME "," expression ")" block
             | "for" "(" steps "," expression "," steps ")" block ;
steps        = [ step | "(" step { "also" step } ")" ] ;
step         = a declaration, assignment, update, or expression, without its ";" ;
run          = "run" block { handler } [ "then" block ] ;
handler      = "handle" "(" handled ")" block ;
handled      = type NAME | "[" type { "," type } "]" NAME | type { "," type } ;
```

- `name = value;` both declares and assigns: it updates the nearest existing
  `name`, or declares a new local. A type, `fixed`, `hot`, or `cold` always
  declares. `fixed` comes before `hot` or `cold`.
- `+=` and `-=` work on plain names only; `++` and `--` also on fields. There
  is no `*=` or `/=`.
- `x[i] = value;` is not an assignment: change an array element with
  `modify(i, value)`.
- A `function` inside a block declares a local function that closes over the
  variables around it; see [functions as values](language.md#functions-as-values).
- `run` needs at least one `handle` or a `then`. `break` and `continue` must be
  inside a loop, and not inside a function nested in it.

## Expressions

```text
expression   = or ;
or           = and { "or" and } ;
and          = not { "and" not } ;
not          = "not" not | comparison ;
comparison   = sum { ( "==" | "!=" | "<" | "<=" | ">" | ">=" ) sum } ;
sum          = product { ( "+" | "-" ) product } ;
product      = sign { ( "*" | "/" | "%" ) sign } ;
sign         = ( "+" | "-" ) sign | power ;
power        = postfix [ "**" sign ] ;
postfix      = primary { "." NAME | "(" [ arguments ] ")" | "[" expression "]" } ;
arguments    = expression { "," expression } [ "," "reverse" "=" expression ] ;
primary      = NUMBER | STRING | "true" | "false" | "fasle" | "null" | NAME
             | "(" expression ")" | lambda ;
lambda       = "func" "(" [ params ] ")" "=>" expression ;
```

### Precedence

From loosest to tightest; operators on one row share a precedence and group
left to right, except `**`.

| Level | Operators | Notes |
| --- | --- | --- |
| 1 | `or` | Short-circuits; the result is a boolean |
| 2 | `and` | Short-circuits; the result is a boolean |
| 3 | `not` | `not a == b` is `not (a == b)` |
| 4 | `==` `!=` `<` `<=` `>` `>=` | `a < b < c` compares the boolean `a < b` with `c`, a `TypeError`; write `a < b and b < c` |
| 5 | `+` `-` | |
| 6 | `*` `/` `%` | |
| 7 | unary `+` `-` | `-2 ** 2` is `-(2 ** 2)`, which is `-4` |
| 8 | `**` | Groups right to left: `2 ** 3 ** 2` is `2 ** 9` |
| 9 | `.name` `(...)` `[...]` | Member, call, and index, left to right |

A lambda's body is a whole expression, so it extends as far as it can:
`func(int x) => x + 1` inside a call ends at the next `,` or `)`.
`reverse=` is the only named argument, and only in `slice` and `lenslice`.

## Types

```text
type         = simple | "[" type { "," type } "]" | collection ;
simple       = "int" | "longint" | "float" | "string" | "bool" | "any"
             | "date" | "datetime" | "duration"
             | "Function" | "Family" | NAME | NAME "." NAME ;
collection   = ( "array" | "set" ) [ "[" type "]" ]
             | "pair" [ "[" type "," type "]" ]
             | ( "map" | "dict" ) [ "[" type "," type "]" ] ;
```

`[int, float]` is a union: a value of any listed type. A family name, or
`alias.Family` for one from a package, accepts instances of that family and of
families inheriting from it. `Function` and `Family` are for parameters only.

## Lexical structure

- **Layout** does not matter: statements end with `;` and blocks are braces, so
  a program may be split across lines or packed onto one. Lines are only used
  to report where an error is.
- **Comments** are `// to the end of the line` and `/* ... */`, which do not nest.
- **Names** start with an ASCII letter or `_`, then letters, digits, and `_`.
  A name starting with `_` is private to its package, as is a function or
  family marked `@Decorator.private`. A method name starting with `_` is only a
  convention; privacy for methods is `@Decorator.private` as well.
- **Keywords**: `func` `family` `takepkg` `ret` `if` `elif` `else` `while`
  `repeat` `for` `break` `continue` `run` `handle` `then` `fixed` `hot` `cold`
  `and` `or` `not` `also` `true` `false` `fasle` `null` `me`.
- **Numbers**: `42`, `3.5`, `1e9`, `2.5E-3`. A whole number is an `int` when it
  fits in 32 bits (-2,147,483,648 to 2,147,483,647) and a `longint` otherwise,
  so `3000000000 + 1` is the longint `3000000001`; one with a `.` or an
  exponent is a `float`. There are no hexadecimal, binary, or digit-separator
  forms, and a leading `-` is the unary operator.
- **Strings**: `"..."`, `'...'`, and `"""..."""`, which may span lines and
  contain `"`. Escapes: `\n` `\r` `\t` `\b` `\f` `\v` `\0`, `\xHH`, `\uHHHH`,
  `\u{H...}`, and a backslash before any other character stands for that
  character, so `\"` and `\\` work in every kind of string.
- **Limits**: one expression may nest at most 1,000 levels deep, counting
  parentheses, calls, and unary operators, and chain at most 10,000 operators
  in a row, as in `a + b + c`. Beyond either, compiling stops with an error,
  so no file, however it was made, can crash `marslang check` or an editor
  that runs it.

## Not in the grammar

These are recognised only to report a clear error, or not at all:

| Written | What happens |
| --- | --- |
| `a ? b : c` | Compile error; use `if` |
| `x in items` | Compile error; use `items.has(x)` |
| `[1, 2, 3]` as a value | Compile error; use `arr(1, 2, 3)` |
| `a // b` | `//` starts a comment, so the rest of the line disappears; use `math.div_floor(a, b)` |
| `"x is {x}"` | An ordinary string: braces are kept as written |
| `func f(int a, int b = 2)` | Compile error; parameters have no defaults |
| `func f(int a) -> int` | Compile error; there are no return type annotations |
| `func(int x){ ... }` | Compile error; an anonymous function's body is one expression |
| `:=`, bitwise operators | Excluded from the language |
