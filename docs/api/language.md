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
`float`, `bigint` (a whole number of any size, [std.bigint](std-bigint.md)),
`string`, `array`, `set`, `pair`, `map`/`dict`, and the calendar kinds
`date`, `datetime`, and `duration` ([dates and times](dates.md)). `any` accepts every
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
conversion behavior: for example, assigning `1` to a `float` binding yields a float,
and to a `bigint` binding a bigint. Conversions only widen: a bigint passed to an
`int`, `longint`, or `float` parameter is a `TypeError` until it is converted with
`int(...)`, `longint(...)`, or `float(...)`.
For exact matching between arguments, see [std.math](std-math.md).

Arithmetic follows the runtime operand kinds. An operation involving a float
produces a float; non-integral number division also produces a float. Integer
arithmetic checks overflow, including through union-typed parameters: an int or
longint result that does not fit raises `RangeError` and never becomes a bigint.
An operation involving a bigint and an int or longint produces a bigint, exactly.
Mixed longint/float and bigint/float arithmetic require an explicit conversion.
Equality and ordering compare numbers by exact value across all kinds: `1 == 1.0`,
`1 == longint(1)`, `bigint(1) == 1.0`, and `longint(5) < 10` are all true, and equal
numbers are the same set element or map key. Strings are never coerced to numbers
for equality.

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

## Functions as values

A function is a value: pass it, store it, and call it later. `func(type name) =>
expression` makes one without a name, and a `func` written inside a block is a
named one that only that block can see.

```mars
func apply(Function f, int value) => f(value);

func counter(){
    count = 0;
    func bump(){
        count = count + 1;
        ret count;
    }
    ret bump;
}

func m{
    double = func(int x) => x * 2;
    out(apply(double, 5), apply(func(int x) => x + 100, 1));   // 10 101

    next = counter();
    next();
    out(next());                                               // 2
}
```

| Form | Meaning |
| --- | --- |
| `func(int x) => x * 2` | An anonymous function; its body is one expression |
| `func name(...){ ... }` inside a block | A named function local to that block, which can call itself |
| `Function f` | A parameter that takes any function, bound method, or built-in |
| `Family F` | A parameter that takes a family, to construct with `F(...)` |

An anonymous function's body is a single expression. For more, declare a named
`func` in the block and pass its name: `func(int x){ ... }` is an error that
says so.

**Closures.** A function made inside another keeps the variables around it,
not copies of their values: assigning to one inside the function changes it
outside, and a change outside is seen inside. Each call of `counter` above makes
a new `count`, which its `bump` keeps after `counter` returns. Inside a method,
a nested function keeps `me` and may call the family's private methods.

Because the variable itself is kept, functions made in a loop share the loop
variable and see its last value:

```mars
func m{
    fns = arr();
    for (i = 0, i < 3, i++){ fns.add(func(int x) => x + i); }
    out(fns[0](10));    // 13, since i is 3 once the loop ends
}
```

To keep each value, make the function in a function that takes it as a
parameter, which is a new variable on every call.

`Function` and `Family` are parameter types only; `f (Function) = ...` is a
compile error. A nested function cannot be reassigned, and closures that
refer to each other or to themselves are reclaimed like any other cycle.

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

## match

`match` compares one value with a list of patterns and runs the first arm that
fits: a clean replacement for a long `if`/`elif` chain.

```mars
family Shape{}
family Circle(Shape){
    func init(float r){ me.r = r; }
    func width() => me.r * 2;
}

func describe(any value){
    match (value){
        1 => ret "one";
        2, 3 => ret "two or three";
        "quit" => { out("cleaning up"); ret "bye"; }
        int n if (n > 100) => ret f"big {n}";
        string s => ret "text " + s;
        Circle c => ret f"a circle {c.width()} wide";
        Shape => ret "some other shape";
        null => ret "nothing";
        _ => ret "anything else";
    }
}

func m{
    out(describe(1));               // one
    out(describe(3.0));             // two or three
    out(describe(500));             // big 500
    out(describe("hi"));            // text hi
    out(describe(Circle(1.5)));     // a circle 3 wide
    out(describe(Shape()));         // some other shape
    out(describe(null));            // nothing
    out(describe(7));               // anything else
}
```

The value in parentheses, the subject, is evaluated once. The arms are tried
from top to bottom, and the first whose pattern matches, and whose guard holds
if it has one, runs; there is no fall-through to the next arm. When no arm
matches, nothing happens.

An arm is `pattern => body`. The body is one statement, ending in `;`, or a
`{ ... }` block, and starts on the line of its `=>`. `ret`, `break`, `continue`,
and errors behave as in an `if` block: `ret` returns from the function, and
`break` leaves the loop around the match. The last arm may leave out its `;`
before the closing brace.

### Patterns

| Pattern | Matches |
| --- | --- |
| `1`, `-2.5`, `"quit"`, `true`, `null` | A value equal to the literal, as `==` says: `1` matches `1`, `1.0`, `longint(1)`, and `bigint(1)`, but not `"1"` |
| `2, 3, 5` | A value equal to any of the literals |
| `_` | Any value |
| `int n` | A value of that type, bound to `n` for the arm |
| `string` | A value of that type, without a name |
| `[int, float] n` | A value of any of the listed types |
| `Circle c` | An instance of the family `Circle`, or of a family inheriting from it |
| `geo.Circle c` | The same, for a family from the imported package `geo` |
| `pattern if (condition)` | What the pattern matches, when the condition is also true |

The types are `int`, `longint`, `bigint`, `float`, `string`, `bool`, `array`,
`set`, `map` (or `dict`), `pair`, `date`, `datetime`, `duration`, `any`, and
families, including the error families. Literals are numbers, strings, `true`,
`false`, and `null`. A variable is not a pattern; compare with one in a guard:
`_ if (value == limit) => ...`.

A type pattern converts nothing. An annotation turns `1` into `1.0` for a
`float` parameter, but `float f` matches only floats: it does not match `1`, and
`int n` does not match `1.0` or `longint(1)`. Write `[int, float] n` to accept
both. Families work as in annotations: `Shape s` matches instances of `Shape`
and of every family inheriting from it. `any` matches every value, `null`
included. Patterns do not check elements, so there is no `array[int] a`; write
`array a` and check the elements in a guard or in the body.

### Guards and names

```mars
func m{
    k = "outer";
    for (n, arr(-3, 4, 0)){
        match (n){
            int k if (k < 0) => out(f"negative {k}");    // negative -3
            int k if (k % 2 == 1) => out(f"odd {k}");
            int k if (k > 0) => out(f"positive {k}");    // positive 4
            _ => out("zero");                            // zero
        }
    }
    out(k);                                              // outer
}
```

A guard, `if (condition)` after any pattern, is checked only when the pattern
matches; when it is false, the next arm is tried. A type pattern binds its name
before the guard runs, so the guard and the body can both use it.

The name belongs to its arm, as the item of `for (item, ...)` and the error of
`handle(Type e)` belong to theirs: it is a new variable that hides an outer
variable of the same name inside the arm only, and it is unknown after the arm.
Assigning to it in the arm changes the arm's variable, never the outer one.

### match as a value

`match` also makes a value. Each arm's body is then an expression, and the
result is the value of the arm that matches.

```mars
func label(int n) => match (n){
    1 => "one";
    2, 3 => "a few";
    int k if (k > 100) => "lots";
    _ => "some"
};

func m{
    out(label(2), label(500), label(7));                    // a few lots some
    word = match (label(1)){ "one" => "uno"; _ => "?" };
    out(word);                                              // uno
    out(match (word){ "uno" => 1; _ => 0 } + 1);            // 2
}
```

An expression `match` with no matching arm has no value to give, so it raises
`RangeError: no match arm matches 7 (an int)`; end with a `_` arm when every
value needs an answer. It may span several lines after `=`, `ret`, a
function's `=>`, or another arm's `=>`. Inside parentheses, as in a call's
argument, it must fit on one line, since a `{` there cannot follow a line
break; give it a variable first when it is longer.

### Errors

These are compile errors, and each names its line:

| Written | Error |
| --- | --- |
| `match (x){ }` | The match has no arms |
| `1 out("one");` | An arm needs `=>` |
| `_ => ...;` before another arm | The arms after it can never run; so does `any v` without a guard |
| `1 => ...; 2, 1.0 => ...;` | `1` appears twice; a repeat after an arm with a guard is allowed |
| `Cirle c => ...;` | Unknown type |
| `limit => ...;` | Unknown type: a bare name is a type, so compare with a variable in a guard |
| `array[int] a => ...;` | Patterns do not check element types |
| `int, float => ...;` | Only literals are listed with commas; write `[int, float]` |

At run time, an error in an arm's guard or body is reported on the arm's own
line, and an expression `match` without a matching arm raises `RangeError` on
the line of the statement it is in.

`match` is a keyword. It cannot name a variable, parameter, function, family,
loop variable, or handled error, so a program that used it as a name must
rename it (the standard library never did). After a dot it is still an
ordinary name, so a field such as `result.match` keeps working.

### How it works

The parser reads each arm into a pattern, an optional guard, and a body, and
the resolver gives a type pattern's name a scope of its own, as it does for the
item of a `for`. Literal patterns become values when the program is compiled,
so matching one compares the subject with it directly; the `if`/`elif` chain a
match replaces evaluates its variable, and builds each string literal, again
for every comparison, so a match over literals is never slower and usually
faster. A type pattern checks the value's kind, walking the family chain for a
family. All of it is in the Rust interpreter (`src/parser.rs`,
`src/resolve.rs`, and `src/interp.rs`).

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
A method taken as a value inside its family can be stored or passed on, but every
call is checked against the method running at that moment, so it stays private.
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

[std.Error](std-error.md) names the built-in families in one namespace:
`takepkg std.Error;` then `err(Error.RangeError, "too big")`,
`handle(Error.TypeError e)`, and `family ParseError(Error.Base){}`. They are the
same families as the bare names. A family is raised with `err`, never called:
`TypeError("x")` is itself a `TypeError` that says so.

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
`alias.Family` to name an error family from a package, in a handler, in `err`,
or as a parent: `family Timeout(net.NetworkError){}`.

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
package, and dotted names map to subdirectories. A name the program itself does not
provide is looked for next in your package directory, so packages you install are
importable from every program you write.

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

### Your package directory

Packages installed for you live in `marslang_pkgs` in your home directory
(`C:\Users\you\marslang_pkgs`, `/home/you/marslang_pkgs`), or wherever
`MARSLANG_PKGS` points. `marslang pkgs` prints the directory it will use.

The program's own directory is searched first, so a file beside your program
always wins over an installed package of the same name, and a program cannot be
changed by something installed later. Within one package the two never mix: a
package found in your package directory runs the `init.mars` files of its parents
from there as well. When an import fails, the error lists every file that was
looked for, in both places.

Each package is loaded once under its absolute name, however it is written or however
many files import it; its top-level statements run once, before the importing file's.
A package exports its functions, families, and `fixed`/`hot` top-level bindings.
Names that start with `_` are private to the package, and so is a function or
family marked [`@Decorator.private`](std-decorator.md#private-declarations). A package's `m` function is not
run. Circular imports are rejected. `takepkg package = *;` is planned and currently rejected.

Standard packages are written in Marslang under `std/`. The few primitives Marslang
cannot express (platform float functions, raising a named error, reading a value's
kind) come from native packages written in Rust under `std/rs/`, imported as
`takepkg rs.NAME;`. Only standard packages may import `rs.*` packages.

`@Decorator.static`, `@Decorator.class`, `@Decorator.overload`, async, and `match`
are pending. `:=` and native bitwise
operators are excluded. Proposed syntax is not an implemented API.
