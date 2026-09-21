# std.Error

`std.Error` names the built-in error families in one place:

```mars
takepkg std.Error;

family ParseError(Error.Base){}

func parse(string text){
    if (text == ""){ err(ParseError, "empty input"); }
    if (text.len() > 80){ err(Error.RangeError, "input longer than 80 characters"); }
    ret text;
}

func m{
    run{
        parse("");
    } handle(ParseError e){
        out(e);                        // ParseError: empty input
    } handle(Error.Base e){
        out("some other error: " + e.message);
    }
}
```

| Member | Raised for |
| --- | --- |
| `Error.Base` | The base family: every error inherits from it, so `handle(Error.Base)` catches all of them |
| `Error.TypeError` | A value of the wrong type, or a call with the wrong arguments |
| `Error.RangeError` | A number out of range: integer overflow, division by zero, a math domain error |
| `Error.OutOfBoundsError` | A position or length outside a string or collection |
| `Error.SyntaxError` | Text that does not parse, such as `longint("12x")` |

These are the families the interpreter itself raises, not copies, so
`handle(Error.RangeError e)` catches a division by zero and `Error.TypeError` and
the bare `TypeError` are the same family. The bare names keep working without the
import.

## Raising, not calling

A family is raised with `err`, never called:

```mars
err(Error.TypeError, "expected a list");    // raises TypeError: expected a list
Error.TypeError("expected a list");         // TypeError: TypeError is an error family:
                                            // raise it with err(TypeError, message) instead of calling it
```

A family you declare with an `init` can still be constructed and then raised with
`err(instance)`, which is how an error carries extra fields.

## The name `Error`

Importing the package binds `Error` in that file to the package, so the base
family is `Error.Base` there, and `family ParseError(Error.Base){}` extends it.
`handle(Error e)` and `family X(Error){}` still name the base family, because
handlers and parents look for a family first. To keep every bare name as it
was, import the package under another name:

```mars
takepkg std.Error = errors;

func m{
    run{ err(errors.RangeError, "too big"); } handle(Error e){ out(e); }
}
```

Standard packages raise these families through `rs.core.raise(RangeError,
message)`, which takes a family, not its name as a string; it is available to
standard packages only, and it raises built-in families only. Everywhere else,
use `err`.
