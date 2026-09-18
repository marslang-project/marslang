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
`float`, `string`, `array`, `set`, `pair`, and `map`/`dict`. Collection restrictions
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
longint/float arithmetic requires an explicit conversion. Numeric equality remains
value-based (`1 == 1.0`); strings are never coerced to numbers for equality.

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

## Operators

Arithmetic: `+`, `-`, `*`, `/`, `%`, `**`; unary `+` and `-`.
Comparisons: `==`, `!=`, `<`, `<=`, `>`, `>=`.
Boolean operators: `not`, `and`, `or`. `and`/`or` short-circuit and return booleans.
Updates include `=`, `+=`, `-=`, `++`, and `--`.

Equality does not coerce strings into numbers. Containers compare by identity.
String `+` joins two strings. Typed integer overflow raises an error.
Exponentiation is right-associative; arithmetic precedes comparisons, which precede
`not`, then `and`, then `or`.

## Imports and pending syntax

`takepkg module;` and `takepkg module = alias;` have bootstrap support through the
JavaScript backend. The bundled `takepkg std.math;` binds `math`, or use an explicit
alias, and compiles the Marslang library into the program. Recursive filesystem
`.mars` module compilation, exports, wildcard imports,
and a packaged standard library are not complete. Do not assume Python-compatible
module discovery. `takepkg package = *;` is planned and currently rejected.

Decorators, async, `match`, and `run/handle/then` are pending. `:=` and native bitwise
operators are excluded. Proposed syntax is not an implemented API.
