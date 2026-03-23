# Runtime and VM API

## Overview

The runtime executes compiled Marslang programs. Compiled output contains a `PROGRAM = ...` payload and calls:

```python
from marslang.runtime import execute_program
```

## Main entrypoint

### `execute_program(program: dict, invoke_main: bool = True)`

- Loads the serialized program into a `VirtualMachine`.
- Registers built-in Marslang functions and conversions.
- Executes top-level declarations.
- Optionally invokes `m()` if present.

## Runtime data structures

### `MArray`

Implements Marslang arrays and methods such as:

- `add`
- `iget`
- `pop`
- `lpop`
- `iremove`
- `get`
- `rget`
- `remove`
- `modify`
- `change`
- `sort`
- `asort`
- `rev`
- `slice`
- `lenslice`

### `MSet`

Implements Marslang sets with de-duplication plus analogous helper methods.

### `MPair`

Simple mutable two-slot pair object with `.first` and `.second`.

The v0.4 runtime also supports index access for arrays, sets, and pairs where applicable.

## Built-in runtime functions

- `arr(*values)`
- `set(*values)`
- `pair(first, second)`
- `out(*values)`
- `slout(*values)`
- `in_()` (mapped from Marslang `in()`)
- `inln()`
- `CHAR_CNVRT(value)`
- `err(error_type, message)`
- `UNPACK_ARR(values)`

## Environment model

The VM uses nested `Environment` instances for lexical-ish scope handling.

Each environment stores:

- `values`
- `constants`
- a `parent` link

This allows the runtime to:

- resolve variables through parent scopes
- prevent reassignment of `fixed` and `hot` names
- create loop/block/function/method scopes

## Function and family execution

- `FunctionDecl` nodes become Python callables that execute Marslang VM statements.
- `FamilyDecl` nodes become Python classes created with `type(...)`.
- `init` is mapped to `__init__`.
- `me` and `self` are both populated in method-local environments.

## Type enforcement

The v0.4 runtime enforces:

- scalar built-in types such as `int`, `float`, `string`, `char`, and `bool`
- collection restrictions for `array[...]`, `set[...]`, and `pair[..., ...]`
- union types written like `[int, string]`

## Notes

The current VM is intentionally simple and designed for iteration rather than optimization. It is best thought of as a structured interpreter over a compiled Marslang program representation.
