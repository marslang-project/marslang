//! `takepkg rs.json;` - reading and writing JSON (RFC 8259) for `std.json`.
//!
//! Written in Rust rather than Marslang for two reasons: speed, since a JSON
//! document is read one character at a time, and correctness, since Marslang
//! strings count grapheme clusters, which would glue a quote to a combining
//! accent written straight after it, at the start of a string value.

use std::collections::HashSet;
use std::fmt::Write;

use indexmap::IndexMap;

use crate::value::*;

/// Deeper nesting is refused rather than risking the interpreter's stack.
const MAX_DEPTH: usize = 512;

pub fn package() -> Value {
    NativePackage::new("rs.json")
        .function("parse", 1, |args| match &args[0] {
            Value::Str(text) => Parser::new(text, "").document(),
            other => type_err(format!("json.parse expects a string, got {}", other.type_name())),
        })
        // As parse, naming the file in errors: json.load.
        .function("parse_from", 2, |args| match (&args[0], &args[1]) {
            (Value::Str(text), Value::Str(source)) => Parser::new(text, source).document(),
            _ => type_err("json.parse_from expects the text and where it came from"),
        })
        .function("stringify", 2, |args| {
            let indent = match &args[1] {
                Value::Int(n) if (0..=16).contains(n) => *n as usize,
                _ => return range_err("json indent must be an int from 0 to 16"),
            };
            let mut writer = Writer { out: String::new(), indent, open: HashSet::new() };
            writer.value(&args[0], 0)?;
            Ok(Value::str(&writer.out))
        })
        .build()
}

// ----- reading -----

struct Parser {
    chars: Vec<char>,
    at: usize,
    depth: usize,
    /// Where the text came from, such as a file, to start error messages with.
    source: String,
}

impl Parser {
    fn new(text: &str, source: &str) -> Self {
        // A byte-order mark may start a document; RFC 8259 lets readers skip it.
        let text = text.strip_prefix('\u{feff}').unwrap_or(text);
        Parser { chars: text.chars().collect(), at: 0, depth: 0, source: source.to_string() }
    }

    /// Line and column (both from 1) of a position, for error messages.
    fn place(&self, at: usize) -> (usize, usize) {
        let mut line = 1;
        let mut column = 1;
        for &c in &self.chars[..at.min(self.chars.len())] {
            if c == '\n' { line += 1; column = 1; } else { column += 1; }
        }
        (line, column)
    }

    fn fail<T>(&self, at: usize, message: impl std::fmt::Display) -> RResult<T> {
        let (line, column) = self.place(at);
        let from = if self.source.is_empty() { String::new() } else { format!("{}: ", self.source) };
        err(ErrorKind::SyntaxError, format!("{from}JSON line {line}, column {column}: {message}"))
    }

    fn peek(&self) -> Option<char> { self.chars.get(self.at).copied() }

    fn skip_space(&mut self) {
        while matches!(self.peek(), Some(' ' | '\t' | '\n' | '\r')) { self.at += 1; }
    }

    /// What the reader found, for "expected X, found Y".
    fn found(&self) -> String {
        match self.peek() {
            None => "the end of the text".into(),
            Some(c) if c.is_control() => format!("U+{:04X}", c as u32),
            Some(c) => format!("'{c}'"),
        }
    }

    fn document(&mut self) -> RResult<Value> {
        self.skip_space();
        let value = self.value()?;
        self.skip_space();
        if self.at < self.chars.len() {
            return self.fail(self.at, format!("expected the end of the text after the value, found {}", self.found()));
        }
        Ok(value)
    }

    fn value(&mut self) -> RResult<Value> {
        match self.peek() {
            Some('{') => self.nested(Self::object),
            Some('[') => self.nested(Self::array),
            Some('"') => Ok(Value::str(&self.string()?)),
            Some('-' | '0'..='9') => self.number(),
            Some('t') => self.word("true", Value::Bool(true)),
            Some('f') => self.word("false", Value::Bool(false)),
            Some('n') => self.word("null", Value::Null),
            _ => self.fail(self.at, format!("expected a value, found {}", self.found())),
        }
    }

    fn nested(&mut self, read: fn(&mut Self) -> RResult<Value>) -> RResult<Value> {
        if self.depth == MAX_DEPTH {
            return self.fail(self.at, format!("nested more than {MAX_DEPTH} levels deep"));
        }
        self.depth += 1;
        let value = read(self);
        self.depth -= 1;
        value
    }

    fn word(&mut self, word: &str, value: Value) -> RResult<Value> {
        let start = self.at;
        for expected in word.chars() {
            if self.peek() != Some(expected) {
                // "True", "nul", "NaN" and friends all land here.
                return self.fail(start, format!("expected {word}"));
            }
            self.at += 1;
        }
        if self.peek().is_some_and(|c| c.is_alphanumeric()) {
            return self.fail(start, format!("expected {word}"));
        }
        Ok(value)
    }

    fn object(&mut self) -> RResult<Value> {
        self.at += 1;
        let mut items = IndexMap::new();
        self.skip_space();
        if self.peek() == Some('}') { self.at += 1; return Ok(Value::new_map(items, Meta::default())); }
        loop {
            self.skip_space();
            if self.peek() != Some('"') {
                return self.fail(self.at, format!("expected a key in double quotes, found {}", self.found()));
            }
            let key = Value::str(&self.string()?);
            self.skip_space();
            if self.peek() != Some(':') {
                return self.fail(self.at, format!("expected ':' after the key, found {}", self.found()));
            }
            self.at += 1;
            self.skip_space();
            let value = self.value()?;
            // A repeated key keeps its first position and its last value.
            items.insert(Key::of(&key), (key, value));
            self.skip_space();
            match self.peek() {
                Some(',') => {
                    self.at += 1;
                    self.skip_space();
                    if self.peek() == Some('}') { return self.fail(self.at - 1, "a comma before '}': JSON allows no trailing comma"); }
                }
                Some('}') => { self.at += 1; return Ok(Value::new_map(items, Meta::default())); }
                _ => return self.fail(self.at, format!("expected ',' or '}}', found {}", self.found())),
            }
        }
    }

    fn array(&mut self) -> RResult<Value> {
        self.at += 1;
        let mut items = Vec::new();
        self.skip_space();
        if self.peek() == Some(']') { self.at += 1; return Ok(Value::array(items)); }
        loop {
            self.skip_space();
            items.push(self.value()?);
            self.skip_space();
            match self.peek() {
                Some(',') => {
                    self.at += 1;
                    self.skip_space();
                    if self.peek() == Some(']') { return self.fail(self.at - 1, "a comma before ']': JSON allows no trailing comma"); }
                }
                Some(']') => { self.at += 1; return Ok(Value::array(items)); }
                _ => return self.fail(self.at, format!("expected ',' or ']', found {}", self.found())),
            }
        }
    }

    fn string(&mut self) -> RResult<String> {
        let start = self.at;
        self.at += 1;
        let mut text = String::new();
        loop {
            let Some(c) = self.peek() else { return self.fail(start, "this string is never closed") };
            self.at += 1;
            match c {
                '"' => return Ok(text),
                '\\' => text.push(self.escape()?),
                c if (c as u32) < 0x20 => {
                    return self.fail(self.at - 1, format!("U+{:04X} must be written as an escape inside a string", c as u32));
                }
                c => text.push(c),
            }
        }
    }

    fn escape(&mut self) -> RResult<char> {
        let at = self.at - 1;
        let Some(c) = self.peek() else { return self.fail(at, "this string is never closed") };
        self.at += 1;
        Ok(match c {
            '"' => '"', '\\' => '\\', '/' => '/',
            'b' => '\u{8}', 'f' => '\u{c}', 'n' => '\n', 'r' => '\r', 't' => '\t',
            'u' => {
                let high = self.hex4(at)?;
                if (0xDC00..=0xDFFF).contains(&high) {
                    return self.fail(at, format!("\\u{high:04X} is the second half of a surrogate pair with no first half"));
                }
                if (0xD800..=0xDBFF).contains(&high) {
                    // A character above U+FFFF is written as two escapes.
                    if self.peek() != Some('\\') || self.chars.get(self.at + 1) != Some(&'u') {
                        return self.fail(at, format!("\\u{high:04X} is the first half of a surrogate pair; the second half must follow"));
                    }
                    self.at += 2;
                    let low = self.hex4(at)?;
                    if !(0xDC00..=0xDFFF).contains(&low) {
                        return self.fail(at, format!("\\u{high:04X} must be followed by a second half from \\uDC00 to \\uDFFF, not \\u{low:04X}"));
                    }
                    let point = 0x10000 + ((high - 0xD800) << 10) + (low - 0xDC00);
                    return Ok(char::from_u32(point).expect("a surrogate pair is a valid code point"));
                }
                char::from_u32(high).expect("not a surrogate, so a valid code point")
            }
            other => return self.fail(at, format!("\\{other} is not a JSON escape; use \\\\ for a backslash")),
        })
    }

    fn hex4(&mut self, at: usize) -> RResult<u32> {
        let mut value = 0;
        for _ in 0..4 {
            let digit = self.peek().and_then(|c| c.to_digit(16));
            let Some(digit) = digit else { return self.fail(at, "\\u must be followed by four hexadecimal digits") };
            value = value * 16 + digit;
            self.at += 1;
        }
        Ok(value)
    }

    fn number(&mut self) -> RResult<Value> {
        let start = self.at;
        if self.peek() == Some('-') { self.at += 1; }
        match self.peek() {
            Some('0') => {
                self.at += 1;
                if self.peek().is_some_and(|c| c.is_ascii_digit()) {
                    return self.fail(start, "a number cannot start with 0 followed by more digits");
                }
            }
            Some('1'..='9') => self.digits(),
            _ => return self.fail(self.at, format!("expected a digit, found {}", self.found())),
        }
        let mut whole = true;
        if self.peek() == Some('.') {
            whole = false;
            self.at += 1;
            if !self.peek().is_some_and(|c| c.is_ascii_digit()) {
                return self.fail(self.at, format!("expected a digit after '.', found {}", self.found()));
            }
            self.digits();
        }
        if matches!(self.peek(), Some('e' | 'E')) {
            whole = false;
            self.at += 1;
            if matches!(self.peek(), Some('+' | '-')) { self.at += 1; }
            if !self.peek().is_some_and(|c| c.is_ascii_digit()) {
                return self.fail(self.at, format!("expected a digit in the exponent, found {}", self.found()));
            }
            self.digits();
        }
        let text: String = self.chars[start..self.at].iter().collect();
        if whole {
            // The same rule as literals in a program: int when it fits in 32 bits.
            return match text.parse::<i64>() {
                Ok(n) if i32::try_from(n).is_ok() => Ok(Value::Int(n)),
                Ok(n) => Ok(Value::Long(n)),
                Err(_) => range_err(format!("the JSON number {text} is beyond longint; write it as a string to keep every digit")),
            };
        }
        let value: f64 = text.parse().expect("the digits were checked");
        if !value.is_finite() { return range_err(format!("the JSON number {text} is too large for a float")); }
        Ok(Value::Float(value))
    }

    fn digits(&mut self) {
        while self.peek().is_some_and(|c| c.is_ascii_digit()) { self.at += 1; }
    }
}

// ----- writing -----

struct Writer {
    out: String,
    /// Spaces per level; 0 writes everything on one line with no spaces.
    indent: usize,
    /// Containers being written, to refuse one that contains itself.
    open: HashSet<usize>,
}

impl Writer {
    fn newline(&mut self, level: usize) {
        if self.indent > 0 {
            self.out.push('\n');
            self.out.push_str(&" ".repeat(self.indent * level));
        }
    }

    fn value(&mut self, value: &Value, level: usize) -> RResult<()> {
        match value {
            Value::Null => self.out.push_str("null"),
            Value::Bool(b) => self.out.push_str(if *b { "true" } else { "false" }),
            Value::Int(n) | Value::Long(n) => { let _ = write!(self.out, "{n}"); }
            Value::Float(f) if f.is_finite() => {
                // {:?} is the shortest text that reads back as the same float
                // and always has '.' or 'e', so 1.0 stays a float when read back.
                let _ = write!(self.out, "{f:?}");
            }
            Value::Float(_) => return range_err("JSON has no infinity or NaN; write it as a string or null"),
            Value::Str(text) => self.string(text),
            // JSON has no dates, so they are written as ISO 8601 strings, which
            // date(), datetime(), and duration() read back.
            Value::Date(d) => self.string(&crate::date::show_date(d)),
            Value::DateTime(t) => self.string(&crate::date::show_datetime(t)),
            Value::Duration(d) => self.string(&crate::date::iso_duration(d)),
            Value::Array(array) => {
                let items = array.items.borrow().clone();
                self.enter(value)?;
                self.out.push('[');
                for (i, item) in items.iter().enumerate() {
                    if i > 0 { self.out.push(','); }
                    self.newline(level + 1);
                    self.value(item, level + 1)?;
                }
                if !items.is_empty() { self.newline(level); }
                self.out.push(']');
                self.leave(value);
            }
            Value::Map(map) => {
                let items: Vec<(Value, Value)> = map.items.borrow().values().cloned().collect();
                self.enter(value)?;
                self.out.push('{');
                for (i, (key, item)) in items.iter().enumerate() {
                    let Value::Str(key) = key else {
                        return type_err(format!("JSON object keys are strings; this map has the key {} ({})", to_display_string(key), key.type_name()));
                    };
                    if i > 0 { self.out.push(','); }
                    self.newline(level + 1);
                    self.string(key);
                    self.out.push_str(if self.indent > 0 { ": " } else { ":" });
                    self.value(item, level + 1)?;
                }
                if !items.is_empty() { self.newline(level); }
                self.out.push('}');
                self.leave(value);
            }
            Value::Set(_) => return type_err("JSON has no sets; write an array of the items instead"),
            Value::Pair(_) => return type_err("JSON has no pairs; write arr(p.first, p.second) or a map instead"),
            Value::Instance(instance) => {
                return type_err(format!("a {} instance cannot be written as JSON; build a map of the fields to write", instance.family.name));
            }
            Value::Func(_) | Value::Package(_) => {
                return type_err(format!("a {} cannot be written as JSON", value.type_name()));
            }
        }
        Ok(())
    }

    fn enter(&mut self, value: &Value) -> RResult<()> {
        let id = value.object_id().expect("containers have an identity");
        if !self.open.insert(id) {
            return type_err(format!("this {} contains itself, so it cannot be written as JSON", value.type_name()));
        }
        Ok(())
    }

    fn leave(&mut self, value: &Value) {
        self.open.remove(&value.object_id().expect("containers have an identity"));
    }

    fn string(&mut self, text: &str) {
        self.out.push('"');
        for c in text.chars() {
            match c {
                '"' => self.out.push_str("\\\""),
                '\\' => self.out.push_str("\\\\"),
                '\n' => self.out.push_str("\\n"),
                '\r' => self.out.push_str("\\r"),
                '\t' => self.out.push_str("\\t"),
                '\u{8}' => self.out.push_str("\\b"),
                '\u{c}' => self.out.push_str("\\f"),
                c if (c as u32) < 0x20 => { let _ = write!(self.out, "\\u{:04x}", c as u32); }
                c => self.out.push(c),
            }
        }
        self.out.push('"');
    }
}
