//! `takepkg rs.string;` - text primitives for `std.strings`. Positions and
//! widths count grapheme clusters, the same unit as `len` and `lenslice`.

use unicode_segmentation::UnicodeSegmentation;

use crate::value::*;

pub fn package() -> Value {
    NativePackage::new("rs.string")
        .function("split", 2, |args| {
            let (text, separator) = (text(&args[0])?, text(&args[1])?);
            if separator.is_empty() { return range_err("split separator must not be empty"); }
            Ok(strings(text.split(separator)))
        })
        .function("split_whitespace", 1, |args| Ok(strings(text(&args[0])?.split_whitespace())))
        .function("lines", 1, |args| Ok(strings(text(&args[0])?.lines())))
        .function("join", 2, |args| {
            let Value::Array(items) = &args[0] else { return type_err("join expects an array of strings") };
            let separator = text(&args[1])?;
            let mut joined = String::new();
            for (i, item) in items.items.borrow().iter().enumerate() {
                let Value::Str(part) = item else { return type_err("join expects an array of strings") };
                if i > 0 { joined.push_str(separator); }
                joined.push_str(part);
            }
            Ok(Value::str(&joined))
        })
        .function("trim", 1, |args| Ok(Value::str(text(&args[0])?.trim())))
        .function("trim_start", 1, |args| Ok(Value::str(text(&args[0])?.trim_start())))
        .function("trim_end", 1, |args| Ok(Value::str(text(&args[0])?.trim_end())))
        .function("starts_with", 2, |args| Ok(Value::Bool(text(&args[0])?.starts_with(text(&args[1])?))))
        .function("ends_with", 2, |args| Ok(Value::Bool(text(&args[0])?.ends_with(text(&args[1])?))))
        .function("contains", 2, |args| Ok(Value::Bool(text(&args[0])?.contains(text(&args[1])?))))
        .function("find", 2, |args| {
            let (haystack, needle) = (text(&args[0])?, text(&args[1])?);
            Ok(position(haystack, haystack.find(needle)))
        })
        .function("rfind", 2, |args| {
            let (haystack, needle) = (text(&args[0])?, text(&args[1])?);
            Ok(position(haystack, haystack.rfind(needle)))
        })
        .function("replace", 3, |args| {
            let (haystack, old, new) = (text(&args[0])?, text(&args[1])?, text(&args[2])?);
            if old.is_empty() { return range_err("replace needs a non-empty text to replace"); }
            Ok(Value::str(&haystack.replace(old, new)))
        })
        .function("upper", 1, |args| Ok(Value::str(&text(&args[0])?.to_uppercase())))
        .function("lower", 1, |args| Ok(Value::str(&text(&args[0])?.to_lowercase())))
        .function("repeated", 2, |args| {
            let text = text(&args[0])?;
            match &args[1] {
                Value::Int(n) if *n >= 0 => {
                    if text.len().saturating_mul(*n as usize) > 1 << 30 { return range_err("repeated text would be too large"); }
                    Ok(Value::str(&text.repeat(*n as usize)))
                }
                _ => range_err("repeat count must be a nonnegative int"),
            }
        })
        .build()
}

fn text(value: &Value) -> RResult<&str> {
    match value {
        Value::Str(s) => Ok(s),
        other => type_err(format!("expected string, got {}", other.type_name())),
    }
}

fn strings<'a>(parts: impl Iterator<Item = &'a str>) -> Value {
    Value::array(parts.map(Value::str).collect())
}

/// A byte offset as a grapheme index (clusters before it), or -1.
fn position(text: &str, offset: Option<usize>) -> Value {
    Value::Int(match offset {
        Some(offset) => text.grapheme_indices(true).take_while(|(start, _)| *start < offset).count() as i64,
        None => -1,
    })
}
