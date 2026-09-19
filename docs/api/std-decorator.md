# std.Decorator

`std.Decorator` is a namespace of decorator markers written above family methods.
Importing it makes `@Decorator.NAME` available; the package has no members to call.

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
| `@Decorator.private` | Implemented | Callable only from methods of the declaring family |
| `@Decorator.subclass` | Implemented | Callable from methods of the declaring family and of families inheriting from it |
| `@Decorator.static` | Planned | Method called on the family, without an instance |
| `@Decorator.class` | Planned | Method that receives the family |
| `@Decorator.overload` | Planned | Several functions with one name, chosen by argument types |

Rules and errors are described under [private methods](language.md#private-methods).
Using a decorator without importing `std.Decorator`, an unknown decorator, a planned
one, both `private` and `subclass` on one method, a private `init`, or a decorator
outside a family is a compile error.
