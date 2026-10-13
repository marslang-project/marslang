//! `takepkg rs.format;` - rounding a number to decimal places, and the
//! width of text in a terminal, for `std.format`.
//!
//! Native because rounding correctly needs the exact decimal value of a
//! float: 2.675 is stored as 2.67499999..., so it rounds to 2.67, and
//! multiplying by 100 and rounding in Marslang would get such cases wrong.
//! Rust's formatter works from the exact value. Everything built on these,
//! such as grouping thousands and padding, is Marslang, in `std/format.mars`.

use crate::value::*;

/// More places than any float has digits for, yet small enough that a
/// mistake cannot ask for a string of millions of zeros.
const MAX_PLACES: i64 = 100;

pub fn package() -> Value {
    NativePackage::new("rs.format")
        // "3.14": the number rounded to `places` decimals, every place written.
        .function("fixed", 2, |args| {
            let places = places(&args[1], "format.fixed")?;
            Ok(Value::str(&match &args[0] {
                Value::Int(n) | Value::Long(n) if places == 0 => n.to_string(),
                Value::Int(n) | Value::Long(n) => format!("{n}.{}", "0".repeat(places)),
                Value::Float(x) if !x.is_finite() => format_float(*x),
                Value::Float(x) => unsigned_zero(format!("{x:.places$}")),
                other => return type_err(format!("format.fixed expects a number, got {}", other.type_name())),
            }))
        })
        // "1.23e+4": one digit before the point, `places` after, then the power of ten.
        .function("scientific", 2, |args| {
            let places = places(&args[1], "format.scientific")?;
            let x = match &args[0] {
                Value::Int(n) | Value::Long(n) => *n as f64,
                Value::Float(x) if !x.is_finite() => return Ok(Value::str(&format_float(*x))),
                Value::Float(x) => *x,
                other => return type_err(format!("format.scientific expects a number, got {}", other.type_name())),
            };
            let text = format!("{x:.places$e}");
            // Rust writes 1.23e4 and 1.23e-4; Marslang, like JavaScript, writes the + too.
            let (mantissa, exponent) = text.split_once('e').unwrap();
            let exponent = if exponent.starts_with('-') { exponent.to_string() } else { format!("+{exponent}") };
            Ok(Value::str(&unsigned_zero(format!("{mantissa}e{exponent}"))))
        })
        // Terminal columns: most characters take one; Chinese, Japanese, and
        // Korean characters and most emoji take two; combining marks none.
        // ANSI escape sequences (the colours of std.terminal) take none.
        .function("width", 1, |args| match &args[0] {
            Value::Str(text) => Ok(Value::Int(unicode_width::UnicodeWidthStr::width(without_escapes(text).as_ref()).min(i32::MAX as usize) as i64)),
            other => type_err(format!("format.width expects a string, got {}", other.type_name())),
        })
        .build()
}

/// The text without its CSI escape sequences: ESC `[`, parameter bytes (0-?),
/// intermediate bytes (space to /), and a final byte (@ to ~), as std.terminal's
/// `strip` finds them. An ESC that does not start a whole sequence stays.
fn without_escapes(text: &str) -> std::borrow::Cow<'_, str> {
    if !text.contains('') { return std::borrow::Cow::Borrowed(text); }
    let bytes = text.as_bytes();
    let mut kept = String::with_capacity(text.len());
    let mut copied = 0;
    let mut at = 0;
    while at < bytes.len() {
        if bytes[at] == 0x1b && bytes.get(at + 1) == Some(&b'[') {
            let mut end = at + 2;
            while end < bytes.len() && (0x30..=0x3f).contains(&bytes[end]) { end += 1; }
            while end < bytes.len() && (0x20..=0x2f).contains(&bytes[end]) { end += 1; }
            if end < bytes.len() && (0x40..=0x7e).contains(&bytes[end]) {
                kept.push_str(&text[copied..at]);
                copied = end + 1;
                at = end + 1;
                continue;
            }
        }
        at += 1;
    }
    kept.push_str(&text[copied..]);
    std::borrow::Cow::Owned(kept)
}

fn places(value: &Value, name: &str) -> RResult<usize> {
    match value {
        Value::Int(n) if (0..=MAX_PLACES).contains(n) => Ok(*n as usize),
        Value::Int(n) => range_err(format!("{name} takes 0 to {MAX_PLACES} decimal places, got {n}")),
        other => type_err(format!("{name} expects a whole number of decimal places, got {}", other.type_name())),
    }
}

/// "0.00" rather than "-0.00": a tiny negative number rounded to nothing
/// shows no sign, as people write it.
fn unsigned_zero(text: String) -> String {
    match text.strip_prefix('-') {
        Some(rest) if rest.split('e').next().unwrap_or("").chars().all(|c| matches!(c, '0' | '.')) => rest.to_string(),
        _ => text,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixed(x: Value, places: i64) -> String {
        let package = package();
        let Value::Package(package) = package else { unreachable!() };
        let Some(Value::Func(f)) = package.members.get("fixed") else { unreachable!() };
        let Callable::Native(f) = f.as_ref() else { unreachable!() };
        match (f.imp)(&[x, Value::Int(places)]).unwrap() { Value::Str(s) => s.to_string(), _ => unreachable!() }
    }

    #[test]
    fn rounds_from_the_exact_value() {
        assert_eq!(fixed(Value::Float(2.675), 2), "2.67");
        assert_eq!(fixed(Value::Float(1.005), 2), "1.00");
        assert_eq!(fixed(Value::Float(0.125), 2), "0.12");
        assert_eq!(fixed(Value::Float(-0.001), 2), "0.00");
        assert_eq!(fixed(Value::Long(9007199254740993), 1), "9007199254740993.0");
    }

    #[test]
    fn escape_sequences_are_not_text() {
        assert_eq!(without_escapes("plain"), "plain");
        assert_eq!(without_escapes("[1m[31mhi[0m"), "hi");
        assert_eq!(without_escapes("a[38;2;1;2;3mb[2J[Hc"), "abc");
        // Not whole sequences: left as they are.
        assert_eq!(without_escapes("[31"), "[31");
        assert_eq!(without_escapes("m"), "m");
        assert_eq!(without_escapes("你[1m好"), "你好");
    }
}
