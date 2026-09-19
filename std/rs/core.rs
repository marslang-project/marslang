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
        .build()
}
