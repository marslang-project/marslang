//! `takepkg rs.string;` - text primitives for `std.strings`. Positions and
//! widths count grapheme clusters (characters), the same unit as `len` and
//! `lenslice`. Searches match whole characters only: a match must start and end
//! at character boundaries, so a combining mark inside a character, or the `\n`
//! of a `\r\n` pair, is not found on its own.

use unicode_segmentation::UnicodeSegmentation;

use crate::value::*;

pub fn package() -> Value {
    NativePackage::new("rs.string")
        .function("split", 2, |args| {
            let (text, separator) = (text(&args[0])?, text(&args[1])?);
            if separator.is_empty() { return range_err("split separator must not be empty"); }
            let mut parts = Vec::new();
            let mut start = 0;
            for (at, end) in Text::new(text).matches(separator) {
                parts.push(&text[start..at]);
                start = end;
            }
            parts.push(&text[start..]);
            Ok(strings(parts.into_iter()))
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
        .function("starts_with", 2, |args| {
            let (haystack, prefix) = (text(&args[0])?, text(&args[1])?);
            Ok(Value::Bool(haystack.starts_with(prefix) && Text::new(haystack).is_boundary(prefix.len())))
        })
        .function("ends_with", 2, |args| {
            let (haystack, suffix) = (text(&args[0])?, text(&args[1])?);
            Ok(Value::Bool(haystack.ends_with(suffix) && Text::new(haystack).is_boundary(haystack.len() - suffix.len())))
        })
        .function("contains", 2, |args| Ok(Value::Bool(Text::new(text(&args[0])?).find(text(&args[1])?, false).is_some())))
        .function("find", 2, |args| Ok(index(Text::new(text(&args[0])?).find(text(&args[1])?, false))))
        .function("rfind", 2, |args| Ok(index(Text::new(text(&args[0])?).find(text(&args[1])?, true))))
        .function("replace", 3, |args| {
            let (haystack, old, new) = (text(&args[0])?, text(&args[1])?, text(&args[2])?);
            if old.is_empty() { return range_err("replace needs a non-empty text to replace"); }
            let mut replaced = String::with_capacity(haystack.len());
            let mut start = 0;
            for (at, end) in Text::new(haystack).matches(old) {
                replaced.push_str(&haystack[start..at]);
                replaced.push_str(new);
                start = end;
            }
            replaced.push_str(&haystack[start..]);
            Ok(Value::str(&replaced))
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

fn index(position: Option<usize>) -> Value {
    Value::Int(position.map_or(-1, |i| i as i64))
}

/// A string with its character (grapheme cluster) boundaries.
struct Text<'a> {
    text: &'a str,
    /// Byte offset where each character starts, followed by the text's length.
    bounds: Vec<usize>,
}

impl<'a> Text<'a> {
    fn new(text: &'a str) -> Self {
        let mut bounds: Vec<usize> = text.grapheme_indices(true).map(|(at, _)| at).collect();
        bounds.push(text.len());
        Text { text, bounds }
    }

    fn is_boundary(&self, byte: usize) -> bool {
        self.bounds.binary_search(&byte).is_ok()
    }

    /// Whether `part` occurs as whole characters starting at character `index`.
    fn matches_at(&self, index: usize, part: &str) -> bool {
        let at = self.bounds[index];
        self.text[at..].starts_with(part) && self.is_boundary(at + part.len())
    }

    /// Character index of the first (or last) whole-character occurrence.
    fn find(&self, part: &str, last: bool) -> Option<usize> {
        let count = self.bounds.len() - 1;
        if part.is_empty() { return Some(if last { count } else { 0 }); }
        if last { (0..count).rev().find(|&i| self.matches_at(i, part)) }
        else { (0..count).find(|&i| self.matches_at(i, part)) }
    }

    /// Byte ranges of non-overlapping whole-character occurrences, left to right.
    fn matches(&self, part: &str) -> Vec<(usize, usize)> {
        let mut found = Vec::new();
        let mut i = 0;
        while i + 1 < self.bounds.len() {
            if self.matches_at(i, part) {
                let (at, end) = (self.bounds[i], self.bounds[i] + part.len());
                found.push((at, end));
                i = self.bounds.binary_search(&end).unwrap();
            } else {
                i += 1;
            }
        }
        found
    }
}
