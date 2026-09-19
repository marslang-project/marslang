# std.types

```mars
takepkg std.types;

family Shape{}
family Circle(Shape){}

func m{
    c = Circle();
    out(types.kind(c), types.family_name(c));     // instance Circle
    out(types.is_instance(c, Shape));             // true
}
```

| Function | Result |
| --- | --- |
| `kind(value)` | `"int"`, `"longint"`, `"float"`, `"string"`, `"bool"`, `"null"`, `"array"`, `"set"`, `"map"`, `"pair"`, `"instance"`, `"function"`, or `"package"` |
| `family_name(value)` | The family name of an instance, or `null` |
| `is_instance(value, Family)` | Whether `value` is an instance of `Family` or of a family inheriting from it |
| `is_number(value)` | Whether `value` is an `int`, `longint`, or `float` |
