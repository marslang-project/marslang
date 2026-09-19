# Language syntax

Source files end in `.mars`. Statements end with `;`; blocks use `{...}`.
`//` starts a line comment; `/* ... */` encloses a block comment.

## Variables and types

```mars
func m{
    count (int) = 1;
    count = count + 1;
    if (true){
        cold count = 10;
        out(count);
    }
    out(count);
}
```

Bare assignment updates the nearest existing binding, or creates a local if none
exists. A type annotation or `cold` explicitly declares a local and can shadow an
outer binding. Duplicate declarations in the same scope are errors.

| Form | Meaning |
| --- | --- |
| `name = value;` | Assign an existing binding or create a local |
| `name (type) = value;` | Declare a typed local |
| `cold name = value;` | Explicitly declare a local |
| `fixed name = value;` | Prevent reassignment; recursively freeze container values |
| `hot name = constant;` | Substitute a constant expression; reassignment prohibited |

Place `fixed` before `hot` or `cold` when combining modifiers. The keyword is
`fixed`, not `final`. Direct initialization from a fixed binding inherits its
fixed status. `.copy()` creates a mutable deep copy of a container.

Supported runtime types include `int` (signed 32-bit), `longint` (signed 64-bit),
`float`, `string`, `array`, `set`, `pair`, and `map`/`dict`. `any` accepts every
value, for parameters that work with anything, such as `func push(any item)`. Collection restrictions
use forms such as `array[int]`, `pair[string,int]`, and `map[string,int]`.
This is runtime checking, not a complete static type system.

Marslang is dynamically typed. An unannotated variable can change from an integer
to a string or another kind of value on reassignment. An annotation restricts the
binding at runtime; `fixed` prevents reassignment regardless of type. Plain `=`
already provides automatic type detection; `:=` remains excluded.

Numeric values retain their kind through calls, returns, fields, and containers.
`1.0`, `1e0`, and `float(1)` remain floats even though their value is integral.
Numeric unions prefer the actual kind instead of converting a float to the first
integer alternative. Numeric annotation/conversion boundaries retain their checked
conversion behavior: for example, assigning `1` to a `float` binding yields a float.
For exact matching between arguments, see [std.math](std-math.md).

Arithmetic follows the runtime operand kinds. An operation involving a float
produces a float; non-integral number division also produces a float. Integer
arithmetic checks overflow, including through union-typed parameters. Mixed
longint/float arithmetic requires an explicit conversion. Equality and ordering
compare numbers by exact value across all kinds: `1 == 1.0`, `1 == longint(1)`, and
`longint(5) < 10` are all true, and equal numbers are the same set element or map key.
Strings are never coerced to numbers for equality.

## Functions and families

```mars
func add(int a, int b) => a + b;

family Counter{
    func init(int value){ me.value = value; }
    func next{
        me.value = me.value + 1;
        ret me.value;
    }
}

func m{
    counter = Counter(4);
    out(add(counter.next(),2));
}
```

Parameters use `type name`, separated by commas with no trailing semicolon.
No-argument functions may omit `()`. `m` is the entry function and takes no
parameters. Use `ret value;` to return from a block function. Expression functions
use `=> expression;`.

`family Child(Parent){...}` declares inheritance. `init` is the constructor, and
`me` refers to the current instance. Call a family name to create an instance.

A family name used as a type, such as `func area(Shape s)` or `array[Shape]`,
accepts instances of that family and of every family that inherits from it. The name
refers to the family declared in the same file; use `alias.Family` for a family from
an imported package. Families with the same name in different packages are different types.
Only slicing accepts a named `reverse=` argument; other calls are positional.

## Control flow

| Form | Behavior |
| --- | --- |
| `if (condition){...} elif (condition){...} else {...}` | Conditional branches |
| `while (condition){...}` | Repeat while truthy |
| `repeat count {...}` | Evaluate count once; repeat a nonnegative integer number of times |
| `for (item,iterable){...}` | Visit each item, or each map key |
| `for (i=0,i<10,i++){...}` | Initialization, condition, step; separated by commas |

`break;` exits the nearest loop. `continue;` skips to its next iteration; the step
still runs in a three-part `for`. Multiple initialization/step statements can be
grouped using `also`, for example `for ((i=0 also j=0),i<3,(i++ also j++)){...}`.

False values are `false`, `null`, and numeric zero. Empty strings and containers
are truthy. `fasle` is an accepted alias of `false`.

`inf` is not a keyword. Use `float("inf")`, `float("-inf")`, or `float("nan")`
for non-finite float values; see [math classification](std-math.md#infinity-nan-and-classification).

## Private methods

```mars
takepkg std.Decorator;

family Account{
    func init(){ me.balance = 0; }

    @Decorator.private
    func _audit(string action){ out("audit " + action); }

    @Decorator.subclass
    func _limit() => 100;

    func deposit(int amount){
        me._audit("deposit");        // allowed: a method of Account
        me.balance = me.balance + amount;
    }
}

family Savings(Account){
    func limit() => me._limit();     // allowed: Savings inherits from Account
}

func m{
    Account()._audit("x");           // TypeError: _audit is private to Account
}
```

Decorators are written on the line above a family method (or before `func` on the
same line) and need `takepkg std.Decorator;`; with `takepkg std.Decorator = D;`
write `@D.private`.

| Decorator | Who can call the method |
| --- | --- |
| `@Decorator.private` | Only methods of the family that declares it; not inheriting families |
| `@Decorator.subclass` | Methods of the declaring family and of every family inheriting from it |

Anything else, including top-level code and free functions, gets a `TypeError`,
both when calling the method and when taking it as a value (`f = obj._audit;`).
Methods without a decorator are public. Fields are always public; the `_name`
convention marks fields meant for internal use. `init` cannot be private.

## Error handling

```mars
family ParseError(Error){}

func parse(string text){
    if (text == ""){ err(ParseError, "empty input"); }
    ret text;
}

func m{
    run{
        parse("");
    } handle(ParseError e){
        out(e.message);            // empty input
    } handle(TypeError, RangeError){
        out("a built-in error");
    } then{
        out("always runs");
    }
}
```

Errors are families. `Error` is the base family, and `TypeError`, `RangeError`,
`OutOfBoundsError`, and `SyntaxError` inherit from it. Declare your own with
`family Name(Error){}` or inherit from a built-in kind. Every error has a `message`
field and prints as `Name: message`.

| Form | Meaning |
| --- | --- |
| `err(Family, message)` | Raise a new error of that family |
| `err(e)` | Raise a caught error again |
| `handle(Type e){...}` | Catch `Type` or any family inheriting from it, bound to `e` |
| `handle([Type1, Type2] e){...}` | Catch any listed family, bound to `e` |
| `handle(Type1, Type2){...}` | Catch any listed family without a name |
| `then{...}` | Optional cleanup |
| `lasterr()` | The most recently handled error, or `null` |

`run` needs at least one `handle` or a `then`. Handlers are checked top to bottom
and the first match runs; unmatched errors continue outward. `then` always runs
last: after the body, after a handler, while an unhandled error propagates, and when
the block exits early with `ret`, `break`, or `continue`. An error, `ret`, `break`,
or `continue` inside `then` replaces the pending outcome. Errors raised by the
interpreter itself, such as division by zero, are caught the same way. Use
`alias.Family` to name an error family from a package.

## Operators

Arithmetic: `+`, `-`, `*`, `/`, `%`, `**`; unary `+` and `-`.
Comparisons: `==`, `!=`, `<`, `<=`, `>`, `>=`.
Boolean operators: `not`, `and`, `or`. `and`/`or` short-circuit and return booleans.
Updates include `=`, `+=`, `-=`, `++`, and `--`.

Equality does not coerce strings into numbers. Containers compare by identity.
String `+` joins two strings. Typed integer overflow raises an error.
Exponentiation is right-associative; arithmetic precedes comparisons, which precede
`not`, then `and`, then `or`.

## Packages

`takepkg name;` imports a package and binds it to the last segment of its name;
`takepkg name = alias;` chooses the binding. Members are read with `alias.member`
and are read-only.

Packages work much like Python's. Absolute names resolve from the program's root
directory (the main file's directory); a directory containing `init.mars` is a
package, and dotted names map to subdirectories.

| Form | Loads |
| --- | --- |
| `takepkg std.math;` | A standard package, written in Marslang and built into `marslang` |
| `takepkg util;` | `util/init.mars` if `util/` is a package, otherwise `util.mars` |
| `takepkg shapes.circle;` | `shapes/circle/init.mars` or `shapes/circle.mars` |
| `takepkg .vec;` | `vec` in the current package (a sibling module) |
| `takepkg ..helpers;` | `helpers` in the parent package; each extra dot goes up one level |

A module file belongs to the package of its directory, and an `init.mars` file
is its directory's package. Relative imports need a parent package: the main program
and top-level files use absolute names, and going above the top-level package is an
error. Importing `a.b.c` first runs `a/init.mars` and `a/b/init.mars` when
they exist. A package may import its own modules from `init.mars`. The name `init`
is reserved for these files, so no module can be named `init`.

Each package is loaded once under its absolute name, however it is written or however
many files import it; its top-level statements run once, before the importing file's.
A package exports its functions, families, and `fixed`/`hot` top-level bindings.
Names that start with `_` are private to the package. A package's `m` function is not
run. Circular imports are rejected. `takepkg package = *;` is planned and currently rejected.

Standard packages are written in Marslang under `std/`. The few primitives Marslang
cannot express (platform float functions, raising a named error, reading a value's
kind) come from native packages written in Rust under `std/rs/`, imported as
`takepkg rs.NAME;`. Only standard packages may import `rs.*` packages.

`@Decorator.static`, `@Decorator.class`, `@Decorator.overload`, async, and `match`
are pending. `:=` and native bitwise
operators are excluded. Proposed syntax is not an implemented API.
