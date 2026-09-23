//! `takepkg rs.memory;` - where a value lives, whether two names are one
//! object, and the collector `std.memory` exposes.

use std::rc::Rc;

use crate::value::*;

/// The address of a value that lives behind a pointer, or `None` for one
/// stored inside the value itself (numbers, booleans, and null).
fn address(value: &Value) -> Option<usize> {
    match value {
        // Rc<str> is a fat pointer; its data address is what identifies it.
        Value::Str(text) => Some(Rc::as_ptr(text) as *const u8 as usize),
        _ => value.object_id(),
    }
}

/// How many references the interpreter holds to a value's object.
fn references(value: &Value) -> Option<usize> {
    Some(match value {
        Value::Str(o) => Rc::strong_count(o),
        Value::Array(o) => Rc::strong_count(o),
        Value::Set(o) => Rc::strong_count(o),
        Value::Map(o) => Rc::strong_count(o),
        Value::Pair(o) => Rc::strong_count(o),
        Value::Instance(o) => Rc::strong_count(o),
        Value::Func(o) => Rc::strong_count(o),
        Value::Package(o) => Rc::strong_count(o),
        _ => return None,
    })
}

fn needs_address<T>(name: &str, value: &Value) -> RResult<T> {
    type_err(format!(
        "std.memory.{name} needs a value that lives behind a pointer; a {} is stored inside the value itself",
        value.type_name()
    ))
}

pub fn package() -> Value {
    NativePackage::new("rs.memory")
        .function("address", 1, |args| match address(&args[0]) {
            Some(at) => Ok(Value::Long(at as i64)),
            None => needs_address("address", &args[0]),
        })
        .function("pointer", 1, |args| match address(&args[0]) {
            Some(at) => Ok(Value::str(&format!("{}@0x{at:x}", args[0].type_name()))),
            None => needs_address("pointer", &args[0]),
        })
        // Whether both names hold one object. Numbers and booleans have no
        // identity to compare, so they fall back to being equal.
        .function("same", 2, |args| Ok(Value::Bool(match (address(&args[0]), address(&args[1])) {
            (Some(a), Some(b)) => a == b,
            (None, None) => equal(&args[0], &args[1]),
            _ => false,
        })))
        .function("refs", 1, |args| match references(&args[0]) {
            Some(count) => Ok(Value::Int(count as i64)),
            None => needs_address("refs", &args[0]),
        })
        // Run the cycle collector now; how many objects it freed.
        .function("collect", 0, |_| Ok(Value::Int(crate::gc::collect() as i64)))
        .build()
}
