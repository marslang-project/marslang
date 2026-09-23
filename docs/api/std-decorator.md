# std.Decorator

`std.Decorator` is a namespace of decorator markers written above functions,
families, and family methods.
Importing it makes `@Decorator.NAME` available. The package lists the markers, so
`@Decorator.NAME` is only accepted for a name it exports, and each marker's value
is its own name (`out(Decorator.private)` prints `private`). Decorators are applied
while the program is compiled, so the markers are never called at runtime.

```mars
takepkg std.Decorator;

family Stack{
    func init(){ me._items = arr(); }

    @Decorator.private
    func _top_index() => me._items.len() - 1;

    func peek() => me._items.iget(me._top_index());
}
```

| Decorator | Status | Meaning |
| --- | --- | --- |
| `@Decorator.docstring(text)` | Implemented | Documentation for the function, family, or method below, shown by editors on hover |
| `@Decorator.private` | Implemented | On a method, callable only from methods of the declaring family; on a function or family, kept inside its own package |
| `@Decorator.subclass` | Implemented | Callable from methods of the declaring family and of families inheriting from it |
| `@Decorator.static` | Planned | Method called on the family, without an instance |
| `@Decorator.class` | Planned | Method that receives the family |
| `@Decorator.overload` | Planned | Several functions with one name, chosen by argument types |

The markers live in [std/Decorator.mars](../../std/Decorator.mars); a marker listed
there that the interpreter does not apply yet reports "not implemented yet".

## Docstrings

```mars
takepkg std.Decorator;

@Decorator.docstring("""
    A circle, measured from its radius.

    area() uses pi to five places.
""")
family Circle{
    func init(float r){ me.r = r; }

    @Decorator.docstring("The area inside the circle.")
    func area() => 3.14159 * me.r * me.r;
}

@Decorator.docstring("Adds two whole numbers.")
func add(int a, int b) => a + b;
```

`@Decorator.docstring` takes one string, usually triple-quoted. The first line is
trimmed, the indentation the other lines share is removed, and blank lines at
either end are dropped, so a docstring can be indented to match the code. It
applies to the function, family, or method directly below it, alongside other
decorators, and changes nothing when the program runs.

Every public function, family, and method of the standard library has one.
`marslang symbols file.mars` prints each function, family, and method with its
parameters and docstring, each variable with its type, and the same for the
packages the file imports, as JSON; the
[VS Code extension](https://github.com/marslang-project/vscode-marslang) shows
them when you hover a name.

## Private declarations

On a method, `@Decorator.private` says who may call it: only methods of the same
family. See [private methods](language.md#private-methods).

On a function or family declared outside a family, it says something simpler:
the package does not export it, so importers cannot reach it, while the
package's own code calls it normally.

```mars
takepkg std.Decorator;

@Decorator.private
func check(array values){ ... }

func mean(array values){ check(values); ... }   // fine, same package
```

```mars
takepkg stats;

func m{ stats.check(arr()); }   // TypeError: stats has no member 'check'
```

A leading underscore does the same thing, so `_check` needs no marker; the two
can be combined, and the standard library does, because `_check` also reads as
internal where it is called. The marker is the clearer choice for a helper whose
name reads better without the underscore.

`@Decorator.subclass` stays methods-only: outside a family there is no calling
family to compare against.

## Errors

Using a decorator without importing `std.Decorator`, an unknown decorator, a
planned one, both `private` and `subclass` on one method, a private `init`,
`@Decorator.subclass` outside a family, an argument to `private` or `subclass`,
a docstring that is not one string, or two docstrings on one declaration is a
compile error.
