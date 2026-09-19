//! `takepkg rs.core;` - language primitives for standard packages: raising
//! runtime errors and inspecting a value's kind.

use crate::value::*;

pub fn package() -> Value {
    NativePackage::new("rs.core")
        // core.raise("RangeError", "message") raises a runtime error of that kind.
        .function("raise", 2, |args| {
            let (Value::Str(kind), Value::Str(message)) = (&args[0], &args[1]) else {
                return type_err("rs.core.raise expects a kind and a message string");
            };
            let kind = match kind.as_ref() {
                "TypeError" => ErrorKind::TypeError,
                "RangeError" => ErrorKind::RangeError,
                "OutOfBoundsError" => ErrorKind::OutOfBoundsError,
                "SyntaxError" => ErrorKind::SyntaxError,
                _ => ErrorKind::Error,
            };
            err(kind, message.to_string())
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
