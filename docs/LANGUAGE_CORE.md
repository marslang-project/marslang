# Marslang language contract

Status: syntax and behavior contract, 2026-09-16. This is not a claim that the
Rust compiler implements every feature below.

## Authority and status

The source of language requirements is [Instr.txt](../Instr.txt), read in
chronological order: v0.2 changes override v0.1. Later requests add CLI,
documentation, and correctness requirements. The current backend direction in
[PROJECT_REVIEW.md](PROJECT_REVIEW.md) supersedes the historical Python VM
request: keep the Rust-to-JavaScript bootstrap, then work toward self-hosting.
Historical branch and push requests are not instructions to change branches now.

The user's 2026-09-16 design decisions below extend that source and supersede
earlier open proposals. The subsequent correction confirms **`fixed`, not
`final`**. These additions are specified targets, not implemented compiler features.

The later September 16 syntax revision puts `Decorator` in `std.Decorator` and
replaces semicolon-separated function parameters with commas. The final parameter
has no semicolon. This supersedes historical signatures in `Instr.txt` and the
dated project review. Function names are still required.

Implementation note: the current Rust parser now accepts comma-separated
parameters and rejects the historical semicolon form. Examples and regression
fixtures have been migrated. This does not implement decorators or the other
new runtime features. Trailing commas are not currently accepted.

- **Specified:** an explicit source requirement, whether implemented or not.
- **Deferred:** specified functionality scheduled after the dependable core.
- **Unresolved:** a decision the source does not settle. Existing Python or
  JavaScript behavior is not an implicit decision.

## 1. Source syntax — specified

Sources use `.mrs`. Statements end in `;`. Braced functions and families do not
require a trailing `;`, but accept one. Control-flow block examples omit trailing
semicolons; match arms require them, including braced arms.

Comments use `//` and `/* ... */`; block comments may appear between tokens.
Comment markers inside strings are string content. Both single and double quotes
create strings, not characters. Triple double quotes delimit multiline strings.

```mars
fixed hot pi (float) = 3.14159;
cold count (int) = 0;
name = "Mars";

func area(float r) => pi * r * r;
func choose([int, float] value){
    ret value;
}

family Circle{
    func init(float r){
        me.r = r;
    }
    func size{
        ret area(me.r);
    }
}

func m{
    c = Circle(5);
    out(c.size());
}
```

Variables use `name (type) = value;`; annotations are optional. Function
parameters use comma-separated `type name`, including `[type1, type2] name` for
unions: `func add(int a, int b){}`. There is no semicolon after the last parameter.
Commas inside bracketed types do not separate parameters.
Zero-parameter functions may omit parentheses. Expression functions use `=>`
and a terminating semicolon. `ret` returns a value.

Families use `family Name{}` and inheritance uses `family Child(Parent){}`.
`func init(){}` defines initialization; `me` names the current instance.
The entry function is `func m{}` or `func m(){}`, accepts no inputs, and needs
no explicit `ret 0;`.

### Binding modifiers

`cold x = 5;` and `x = 5;` have the same meaning. `fixed` defines a constant
and precedes `hot` or `cold`; `fixed x = 5;` means `fixed cold x = 5;`.
Reversed modifier order is invalid.

`fixed` makes a value immutable. A shallow assignment from a fixed source
preserves its fixed status even when the destination omits `fixed`; explicitly
writing `fixed` on that destination is recommended. `.copy()` makes a **deep
copy** and does not inherit the source's fixed status. An explicitly fixed
destination is still fixed. `final` is not a new keyword.

```mars
fixed x (int) = 5;
y (int) = x;          // shallow copy; y inherits fixed status
z (int) = x.copy();   // deep copy; z does not inherit fixed status
```

These examples retain Marslang's `name (type)` declarations; the Java-style
declarations in the brainstorming message were illustrative, not a syntax change.
For scalar values, shallow/deep copying need not allocate objects, but the fixed
status distinction still applies. For objects, precise shallow sharing, recursive
immutability, cycles, shared subobjects, and copy behavior for files/functions
remain to be specified. Whether propagation applies to expressions, arguments,
returns, and assignment to an existing binding also remains open.

`hot x (int) = 5;` requires a compile-time constant and forbids reassignment.
Uses are substituted during compilation: `out(x);` becomes the equivalent of
`out(5);`. Emitting a JavaScript `const` alone does not implement `hot`.
`hot func` requests function inlining; its detailed evaluation rules are unresolved.

### Operators and literals

Specified operators: `+ - * / % **`, `== != < <= > >=`, `= += -=`, and
`++ --`. Boolean/loop words include `also`, `or`, `and`, and `not`; the source
also mentions `and/or` without defining its exact meaning. Shifts `<<` and `>>`
are outside the requested language.

`:=` is excluded. Native bitwise operator syntax is also excluded; optional
library-based bit operations are discussed below.

Literals include `true`, `false`, `null`, and the intentional false alias
`fasle`. Other misspellings are not boolean aliases. `null` is reaffirmed by the
2026-09-16 decision; it was already part of the original design. **`inf`** is
added as the infinity keyword. Its type, arithmetic, comparisons, conversions,
and interaction with overflow remain deferred for discussion.

Conditions use truthiness: `false` (including `fasle`), `null`, and numeric zero
are falsy; other values are truthy unless further falsy cases are explicitly
defined. The phrase “falsy statements” does not yet define empty-string,
empty-container, NaN, or user-defined conversion behavior. Do not inherit those
cases from JavaScript or Python automatically.

## 2. Types, collections, and I/O — specified

Basic types are `int` (signed 32-bit), `longint` (signed 64-bit), `string`,
`char`, and `float`. Conversions are `int()`, `longint()`, `string()`, and
`float()`. `CHAR_CNVRT()` converts a single character value to `char`.
JavaScript Number cannot represent every `longint` exactly; implementation must
preserve the specified range without silent precision loss.

### Canonical constructors

The v0.2 names are **`arr(...)`, `set(...)`, and `pair(...)`**. They replace
v0.1 `a(...)`, `s(...)`, and `p(...)`. Keeping the old names as compatibility
aliases is unresolved, not a requirement. Type names remain unchanged:

```mars
nums (array[int]) = arr(3,1,4,1,5);
unique (set[int]) = set(1,2,2);
item (pair[string,int]) = pair("answer",42);
item.second = 43;
```

### Additional builtin containers

Dictionaries/maps and deque are approved **builtin types**, not imports from
`std.containers`. Whether dictionary and map are two types or two names for one
type is unresolved. Constructor spellings, generic annotations, literal syntax,
key/equality rules, ordering, and method APIs are not yet specified. Deque is a
container with operations at both ends; its exact API remains open.

Arrays are variable-length and may contain mixed types without restrictions.
Sets prohibit duplicates. Pairs are mutable, with `first` and `second` fields.
Annotations restrict element types; pairs permit separate types for their two
positions. Restrictions must survive mutation, including writes through aliases.

| Array operation | Specified meaning |
| --- | --- |
| `add(value)` | Append to the end |
| `iget(index)` | Read the indexed element |
| `pop()` / `lpop()` | Remove the first / last element |
| `iremove(index)` | Remove the indexed element |
| `get(value)` / `rget(value)` | Find the first occurrence searching forward / backward |
| `remove(value)` | Remove the first matching value |
| `modify(index,value)` | Replace an indexed element |
| `change(previous,current)` | Replace all matching values |
| `sort()` / `asort()` | Sort descending / ascending |
| `rev()` / `reverse()` | Reverse the collection |
| `slice(start,end)` | Slice over `[start,end)` |
| `lenslice(start,length)` | Slice by start and length |

Sets have similar operations and support slicing; the source recommends sorting
before set slicing. Their exact ordering and operation compatibility are unresolved.
`multilinestr.split()` produces a type-restricted `array[string]`; separator and
empty-line behavior need definition.

`out(...)` appends a newline; `slout(...)` does not. `in()` reads all input into
a multiline string; `inln()` reads one line per call. Multi-argument formatting,
collection formatting, and EOF behavior remain unresolved.

## 3. Control flow, errors, and modules — specified

```mars
if (true) { out(1); }
elif (false) { out(2); }

repeat 5 { out("again"); }
for (i=0, i<5, i++) { out(i); }
for ((i=0 also n=5), (i<5 or n>0), (i++ also n--)) { out(i); }
for (item, items) { out(item); }
while (true) { break; }

match x{
    1 => out("one");
    range(2,5) => { out("two through four"); };
    __ => { out("other"); };
}

run{
    err(Error, "message");
} handle(Error){
    out("handled");
} then{
    out("cleanup");
}

takepkg math_tools;
takepkg package_name_which_is_long = pkg;
```

`repeat x{}` repeats x times. A `for` header has three comma-separated sections:
initializer, condition, change. Grouped expressions inside sections use the
specified words, not comma-separated subexpressions.

The second `for` form is `for (itemname, iterable){}` and visits each item in the
iterable. `while (condition){}` repeats while its condition is truthy.
`break;` exits a loop and `continue;` skips to its next iteration. Nested-loop
targets, iterator protocol, loop variable scope, mutation during iteration,
dictionary iteration contents, and interactions with `then` need precise rules.

Function **declarations** now use comma-separated
parameters (`func add(int a, int b)`); function **calls** also use comma-separated
arguments (`add(1,2)`). Both `for` headers use top-level commas. Commas inside
nested calls do not determine which loop form is used.

Implementation recommendation: parse the two- and three-section headers as
separate loop AST forms. `for` remains control flow; comma-separated syntax does
not make it an ordinary function or require decorator-based overloading first.

`match` stops at the first match. `__` is the default arm, and `range(a,b)`
denotes `[a,b)`.

The canonical cleanup keyword is **`then`**, replacing v0.1 `now_do`.
Cleanup is optional. `handle(Error1, Error2)` accepts multiple error types;
`Error` is the base error type. `err(type,message)` raises an error.
An error array requires unpacking with `&~array` or `UNPACK_ARR()` when passed
to `handle`, rather than being passed directly as a list.

`takepkg` imports a module filename, optionally with an alias, and requires a
semicolon. A module is imported once; it may import supporting files itself.
Resolution, exports, cyclic imports, and per-program module caching need definition.

### Package imports and wildcard imports

`takepkg` supports packages and relative paths. Exact relative-path spelling,
resolution base, and package search rules remain open. Wildcard import syntax is:

```mars
takepkg packageName = *;
takepkg std.containers;
```

The wildcard form imports the module's exported names into the importing scope.
Export selection, private names, collisions, re-exports, and whether submodules
are included must be defined before implementation. It does not automatically
mean recursively importing every file in a package.

### Decorators and overloads

Decorators appear before a function declaration, using a Python-inspired `@`
form. The approved overload marker is:

```mars
takepkg std.Decorator;

@Decorator.overload()
func process(int value) => value;
```

This illustrates annotation placement, not a complete runnable overload example.
The package is **`std.Decorator`**, imported with `takepkg std.Decorator;` and
used as `Decorator`. This corrects the earlier tentative import spelling. The marker
line attaches to the following function rather than being an ordinary statement.

Overload dispatch (compile-time versus runtime, parameter types/arity, ambiguity,
fallbacks, and duplicate signatures), decorator order/evaluation, user-defined
decorators, and a static-method decorator's actual name remain unresolved.
Python's `@staticmethod` was an analogy, not an adopted Marslang decorator name.

### Standard and extension libraries

| Package | Approved purpose and remaining boundary |
| --- | --- |
| `std.containers` | `containers.stack`, `containers.queue`, `containers.priority_queue`; more containers may follow, APIs pending |
| `std.file` | File I/O; implementation and resource/error behavior deferred |
| `std.network` | Networking; subpackages such as `network.http` are tentative, not finalized paths |
| `std.regex`, `std.random`, `std.json` | Planned standard-library facilities; exact public APIs pending |
| `std.asyncio` | Tentative future async package; async/await and the execution model remain deferred |

The distribution may also ship an `ext` extension-library namespace alongside
`std`. `ext.bitopers` is a candidate for library functions such as `BIT_AND()`
and `BIT_OR()` instead of native bitwise operators. Names beyond these examples,
integer widths, signedness, and overflow behavior remain proposals.

Developer tools may start small; working syntax takes priority over extensive
editor, formatter, and debugger support.

## 4. Implementation stages — deferred work is not rejected syntax

1. **Dependable core:** execution harness; one lexer/parser with structured
   expressions and source spans; agreed binding/operator rules; name resolution;
   functions, families, existing conditionals/repeat, basic output, and the
   collection operations needed by the example. Implement constant hot-variable
   substitution explicitly, rather than treating it as fixed binding.
2. **Feature restoration:** complete collection/type enforcement, union parameters,
   scalar conversions, multiline strings, for/match/range, errors/cleanup, hot
   functions, and defined modules. Unsupported constructs should receive clear
   diagnostics until implemented.
3. **Tooling and self-hosting:** verbose CLI, accurate docs and platform wrappers,
   REPL recovery, host I/O and compiler data structures, then a Marslang compiler
   validated against the same corpus as Rust.

The original sorted example, updated to `arr`, prints `1` and `78.53975`.
The repository's current unsorted `hello.mrs` prints `3` and `78.53975` once
its runtime works. These are different fixtures, not conflicting expectations.

## 5. Unresolved decisions

| Area | Decision still required |
| --- | --- |
| Names and scopes | Whether bare `=` declares or assigns; nested-scope writes/shadowing; forward references; function/builtin shadowing policy |
| Constants and hot | Recursive immutability and alias effects; fixed propagation beyond direct copies; deep-copy cycles/sharing/resources; hot evaluation subset, hot-function restrictions, recursion and side effects |
| Operators and numbers | Precedence/associativity, unary operators, exact `also`/`and/or` behavior, short circuiting, increment value semantics, division, overflow, float model, coercion/equality, additional falsy cases, all detailed `inf` behavior |
| Collections and strings | Map/dictionary identity and syntax, deque API, iteration protocol; negative/out-of-range indices, missing-value results, method returns, mixed sorting, set order/equality, constructor conversions, Unicode indexing, escapes and split behavior |
| Control/runtime | `else` contract, repeat count validation, loop scope/control targets and cleanup; decorator dispatch; relative imports, wildcard exports/collisions, library APIs, error subclasses/binding, entrypoint return behavior |

No execution fixture should settle one of these questions implicitly. Mark a
proposed rule as a proposal before adding tests that enforce it.

## 6. Executable contract coverage

`tests/execution.rs` compiles sources using the public Rust API and runs their
JavaScript with Node. Passing cases check exact stdout, successful exit, and
empty stderr. Known gaps are named ignored tests, not accepted compiler behavior.
Run:

```text
cargo test
cargo test --test execution -- --ignored
```

The second command is expected to fail until the recorded gaps are implemented.
Node must be on PATH, or `MARSLANG_NODE` must name its executable. Missing Node
is a test failure, never a silent skip. This initial corpus covers only part of
the contract; it does not establish full language conformance.
