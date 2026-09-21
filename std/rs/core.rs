//! `takepkg rs.core;` - language primitives for standard packages: raising
//! runtime errors and inspecting a value's kind.

use crate::value::*;

pub fn package() -> Value {
    NativePackage::new("rs.core")
        // core.raise(RangeError, "message") raises a built-in error family, the
        // same way err(RangeError, "message") does. A family you declare is
        // raised with err(), which keeps track of which family it is.
        .function("raise", 2, |args| {
            let usage = "rs.core.raise expects a built-in error family and a message, such as core.raise(RangeError, \"message\")";
            let (Value::Func(f), Value::Str(message)) = (&args[0], &args[1]) else { return type_err(usage) };
            let Callable::Family(family) = f.as_ref() else { return type_err(usage) };
            match family.error_kind {
                Some(kind) if family.name == kind.name() => err(kind, message.to_string()),
                Some(_) => type_err(format!("rs.core.raise only raises built-in error families; raise {} with err()", family.name)),
                None => type_err(format!("{} is not an error family; error families inherit from Error", family.name)),
            }
        })
        // core.kind(value): "int", "longint", "float", "string", "bool", "null", "array", ...
        .function("kind", 1, |args| Ok(Value::str(args[0].type_name())))
        // Family name of an instance, or null.
        .function("family_name", 1, |args| Ok(match &args[0] {
            Value::Instance(o) => Value::str(&o.family.name),
            _ => Value::Null,
        }))
        // Whether a value is an instance of a family or of a family inheriting from it.
        .function("is_instance", 2, |args| {
            let Value::Func(f) = &args[1] else { return type_err("is_instance expects a family") };
            let Callable::Family(target) = f.as_ref() else { return type_err("is_instance expects a family") };
            let Value::Instance(o) = &args[0] else { return Ok(Value::Bool(false)) };
            let mut current = Some(o.family.clone());
            while let Some(family) = current {
                if std::rc::Rc::ptr_eq(&family, target) { return Ok(Value::Bool(true)); }
                current = family.parent.clone();
            }
            Ok(Value::Bool(false))
        })
        .build()
}
