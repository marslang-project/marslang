# Marslang Language Reference

## Source files

- Marslang source files use the `.mrs` extension.
- Most statements end with `;`.
- `family { ... }` and block-style `func { ... }` do not require trailing semicolons, though permissive parsing may accept them.

## Variables and modifiers

### Variable declaration

```marslang
value (int) = 5;
name = "mars";
```

### `hot`

Hot values are compile-time constants and are substituted during compilation.

```marslang
hot x (int) = 5;
```

### `cold`

`cold` is accepted as the explicit non-hot form.

### `fixed`

`fixed` marks a declaration as immutable after definition.

```marslang
fixed answer (int) = 42;
```

## Functions

### Block functions

```marslang
func greet(string name;){
    out(name);
}
```

### Expression functions

```marslang
func add(int a; int b;) => a + b;
```

### Hot inline functions

```marslang
hot func square(int x;) => x * x;
```

## Families

Families are Marslang classes.

```marslang
family Person{
    func init(string name;){
        me.name = name;
    }
}
```

- Constructor name: `init`
- Instance self-reference: `me`
- Inheritance: `family Child(Parent){ ... }`

## Built-in collections

### Arrays

```marslang
nums (array[int]) = arr(1, 2, 3);
```

### Sets

```marslang
tags (set[string]) = set("a", "b");
```

### Pairs

```marslang
point (pair[int, int]) = pair(3, 4);
```

## Control flow

### If / elif / else

```marslang
if (x > 0) {
    out("positive");
} elif (x == 0) {
    out("zero");
} else {
    out("negative");
}
```

### Repeat

```marslang
repeat 5 {
    out("loop");
}
```

### For loops

Marslang for loops use the v0.2/v0.3 form:

```marslang
for(i = 0, i < 10, i++){
    out(i);
}
```

You can group multiple init/update expressions with `also`:

```marslang
for((i = 0 also j = 10), i < j, (i++ also j--)){
    out(i);
}
```

### Match

```marslang
match x{
    1 => { out("one"); };
    range(2, 5) => { out("small"); };
    __ => { out("other"); };
}
```

## Error handling

```marslang
run{
    err(Error, "boom");
} handle(Error){
    out("handled");
} then{
    out("cleanup");
}
```

- `run` maps to try behavior.
- `handle(...)` maps to except behavior.
- `then { ... }` is the post-handle/finally stage.

## Input / output

- `out(...)`: print with newline.
- `slout(...)`: print without newline.
- `in()`: read all stdin.
- `inln()`: read one line.

## Comments

- Single-line: `// comment`
- Multi-line: `/* comment */`

## Current implementation notes

The current implementation targets a Python-hosted VM backend. That means the compiler lowers Marslang code into a serialized instruction/program structure that is executed by `marslang.runtime.execute_program`.
