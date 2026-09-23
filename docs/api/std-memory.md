# std.memory

Where a value lives, and whether two names hold one object or two that look
alike.

```mars
takepkg std.memory;

func m{
    a = arr(1, 2);
    b = a;                                        // the same array
    c = arr(1, 2);                                // equal, but a different array
    out(memory.same(a, b), memory.same(a, c));    // true false
    out(memory.pointer(a));                       // array@0x22056ab1c40
}
```

Marslang frees values by itself, so nothing here is needed to write a correct
program. This package is for telling objects apart, for debugging output, and
for seeing how the interpreter keeps values alive.

C's `&x` takes the address of a value and `*p` reads what is there. Marslang
has the first half only: there is no way to reach back into memory through an
address, and no reason to.

| Function | Result |
| --- | --- |
| `address(value)` | The address, as a `longint` |
| `pointer(value)` | The address as text: `array@0x22056ab1c40` |
| `same(a, b)` | Whether two names hold one object |
| `refs(value)` | How many references the interpreter holds to the object |
| `collect()` | Free objects that only refer to each other; how many were freed |

## Which values have an address

Strings, arrays, sets, maps, pairs, instances, functions, and packages live
behind a pointer, so they have one. A number, a boolean, or `null` is stored
inside the value itself, like a C value held in a register, and `address`,
`pointer`, and `refs` raise `TypeError` for them:

```text
TypeError: std.memory.address needs a value that lives behind a pointer; a int
is stored inside the value itself
```

`same` accepts them anyway, because the question still has an answer: for
values without identity it compares them as `==` does, so `memory.same(1, 1.0)`
is `true`, and comparing a number with an object is `false`.

**An address is not a name.** It means nothing between two runs of a program,
and once an object is freed the same number can come back for a different
object. Never store one, and never write one into a file or a key. To ask
whether two values are one object, use `same`.

## same and ==

For arrays, sets, maps, pairs, and family instances, `==` already compares
identity, so `same` agrees with it. Strings are where they differ: `==`
compares the characters, and `same` asks whether it is one string or two.

```mars
out("ab" == "ab");                // true: the same characters
out(memory.same("ab", "ab"));     // false: two strings
out(arr(1, 2) == arr(1, 2));      // false: two arrays
```

## refs and collect

`refs` counts the references the interpreter holds to an object. It answers
"why is this still alive": each name, container, or closure holding the value
counts.

```mars
a = arr(1, 2);
held = memory.refs(a);
b = a;
out(memory.refs(a) > held);   // true: b holds it too
```

The count includes the references the interpreter makes while running the call
itself, so compare two numbers rather than reading one on its own.

A value is freed as soon as nothing refers to it. Objects that refer only to
each other — an array holding itself, a closure holding the variable that holds
the closure — cannot be freed that way, so a collector finds them. It runs on
its own as a program allocates; `collect()` runs it now and returns how many
objects it freed:

```mars
cycle = arr();
cycle.add(cycle);
cycle = null;             // nothing refers to it, but it refers to itself
out(memory.collect());    // 1
```

The wrapper is written in Marslang ([std/memory.mars](../../std/memory.mars))
over the native package [std/rs/memory.rs](../../std/rs/memory.rs).
