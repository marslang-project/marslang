//! `takepkg rs.encoding;` - the byte-level parts of `std.encoding`.
//!
//! Written in Rust because Marslang strings are grapheme clusters with no byte
//! access, while base64, hex, and percent-encoding work on UTF-8 bytes. The
//! tables and loops are hand-written; no crate is needed. Building and
//! splitting query strings is Marslang, on top of `url_encode`/`url_decode`.
//! Every function takes the public name of its caller (`encoding.from_hex`)
//! so that errors can name it. Positions in errors count characters from 1.

use crate::value::*;

const STANDARD: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
const URL_SAFE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";

pub fn package() -> Value {
    NativePackage::new("rs.encoding")
        .function("bytes", 1, |args| Ok(byte_array(text(&args[0], "encoding.bytes")?.as_bytes())))
        .function("from_bytes", 1, |args| {
            let bytes = bytes_of(&args[0], "encoding.from_bytes")?;
            Ok(Value::str(&utf8(bytes, "encoding.from_bytes", "from_bytes")?))
        })
        // base64_encode(text or byte array, url_safe, name)
        .function("base64_encode", 3, |args| {
            let name = name(&args[2]);
            let bytes = match &args[0] {
                Value::Str(s) => s.as_bytes().to_vec(),
                other => bytes_of(other, &name)?,
            };
            let url = args[1].truth();
            Ok(Value::str(&base64_encode(&bytes, url)))
        })
        // base64_decode(text, url_safe, as_text, name)
        .function("base64_decode", 4, |args| {
            let name = name(&args[3]);
            let input = text(&args[0], &name)?;
            let bytes = base64_decode(input, args[1].truth(), &name)?;
            if args[2].truth() {
                Ok(Value::str(&utf8(bytes, &name, "from_base64_bytes")?))
            } else {
                Ok(byte_array(&bytes))
            }
        })
        .function("hex_encode", 1, |args| {
            let mut out = String::new();
            for b in text(&args[0], "encoding.hex")?.bytes() {
                out.push_str(&format!("{b:02x}"));
            }
            Ok(Value::str(&out))
        })
        .function("hex_decode", 1, |args| {
            let name = "encoding.from_hex";
            let input = text(&args[0], name)?;
            let chars: Vec<char> = input.chars().collect();
            if chars.len() % 2 != 0 {
                return syntax(format!("{name}: odd number of hex digits ({}); every byte needs two", chars.len()));
            }
            let mut bytes = Vec::with_capacity(chars.len() / 2);
            for (i, pair) in chars.chunks(2).enumerate() {
                let mut byte = 0u8;
                for (j, c) in pair.iter().enumerate() {
                    match c.to_digit(16) {
                        Some(d) => byte = byte * 16 + d as u8,
                        None => {
                            return syntax(format!(
                                "{name}: character {} is {} which is not a hex digit",
                                i * 2 + j + 1,
                                show(*c)
                            ))
                        }
                    }
                }
                bytes.push(byte);
            }
            Ok(Value::str(&utf8(bytes, name, "from_bytes")?))
        })
        .function("url_encode", 1, |args| {
            let mut out = String::new();
            for b in text(&args[0], "encoding.url")?.bytes() {
                if b.is_ascii_alphanumeric() || matches!(b, b'-' | b'.' | b'_' | b'~') {
                    out.push(b as char);
                } else {
                    out.push_str(&format!("%{b:02X}"));
                }
            }
            Ok(Value::str(&out))
        })
        // url_decode(text, plus_is_space, name, offset): offset is added to the
        // positions in errors, for text cut out of a longer one.
        .function("url_decode", 4, |args| {
            let offset = match &args[3] { Value::Int(n) | Value::Long(n) => *n as usize, _ => 0 };
            let name = name(&args[2]);
            let input = text(&args[0], &name)?;
            let plus = args[1].truth();
            let chars: Vec<char> = input.chars().collect();
            let mut bytes: Vec<u8> = Vec::new();
            let mut i = 0;
            while i < chars.len() {
                let c = chars[i];
                if c == '%' {
                    let digit = |k: usize| chars.get(i + k).and_then(|c| c.to_digit(16));
                    match (digit(1), digit(2)) {
                        (Some(a), Some(b)) => bytes.push((a * 16 + b) as u8),
                        _ => {
                            let shown: String = chars[i..chars.len().min(i + 3)].iter().collect();
                            return syntax(format!(
                                "{name}: bad percent-escape at character {} (\"{shown}\"); % must be followed by two hex digits",
                                i + 1 + offset
                            ));
                        }
                    }
                    i += 3;
                    continue;
                }
                if c == '+' && plus {
                    bytes.push(b' ');
                } else {
                    let mut buf = [0u8; 4];
                    bytes.extend_from_slice(c.encode_utf8(&mut buf).as_bytes());
                }
                i += 1;
            }
            Ok(Value::str(&utf8(bytes, &name, "")?))
        })
        .build()
}

fn syntax<T>(message: String) -> RResult<T> { err(ErrorKind::SyntaxError, message) }

fn name(value: &Value) -> String {
    match value { Value::Str(s) => s.to_string(), _ => "encoding".into() }
}

fn text<'a>(value: &'a Value, name: &str) -> RResult<&'a str> {
    match value {
        Value::Str(s) => Ok(s),
        other => type_err(format!("{name} expects a string, got {}", other.type_name())),
    }
}

fn byte_array(bytes: &[u8]) -> Value {
    Value::array(bytes.iter().map(|b| Value::Int(*b as i64)).collect())
}

fn bytes_of(value: &Value, name: &str) -> RResult<Vec<u8>> {
    let Value::Array(array) = value else {
        return type_err(format!("{name} expects an array of ints 0 to 255, got {}", value.type_name()));
    };
    let mut bytes = Vec::new();
    for (i, item) in array.items.borrow().iter().enumerate() {
        match item {
            Value::Int(n) | Value::Long(n) => {
                if !(0..=255).contains(n) {
                    return range_err(format!("{name}: byte {} is {n}, which is not from 0 to 255", i + 1));
                }
                bytes.push(*n as u8);
            }
            other => return type_err(format!("{name}: byte {} is {}, not an int", i + 1, other.type_name())),
        }
    }
    Ok(bytes)
}

/// Decode UTF-8, or raise SyntaxError saying where it went wrong; `hint` names
/// the function that handles bytes that are not text.
fn utf8(bytes: Vec<u8>, name: &str, hint: &str) -> RResult<String> {
    String::from_utf8(bytes).or_else(|e| {
        let at = e.utf8_error().valid_up_to() + 1;
        let advice = match hint {
            "from_base64_bytes" => " (for binary data, use encoding.from_base64_bytes)",
            _ => "",
        };
        syntax(format!("{name}: the decoded bytes are not valid UTF-8 text; byte {at} is invalid{advice}"))
    })
}

fn show(c: char) -> String {
    if c.is_control() { format!("U+{:04X}", c as u32) } else { format!("\"{c}\"") }
}

fn base64_encode(bytes: &[u8], url: bool) -> String {
    let table = if url { URL_SAFE } else { STANDARD };
    let mut out = String::new();
    for chunk in bytes.chunks(3) {
        let n = (chunk[0] as u32) << 16
            | (*chunk.get(1).unwrap_or(&0) as u32) << 8
            | *chunk.get(2).unwrap_or(&0) as u32;
        out.push(table[(n >> 18) as usize & 63] as char);
        out.push(table[(n >> 12) as usize & 63] as char);
        if chunk.len() > 1 { out.push(table[(n >> 6) as usize & 63] as char); } else if !url { out.push('='); }
        if chunk.len() > 2 { out.push(table[n as usize & 63] as char); } else if !url { out.push('='); }
    }
    out
}

fn base64_decode(input: &str, url: bool, name: &str) -> RResult<Vec<u8>> {
    let table = if url { URL_SAFE } else { STANDARD };
    let other = if url { "+/" } else { "-_" };
    let mut sextets: Vec<u32> = Vec::new();
    let mut padding = 0usize;
    for (i, c) in input.chars().enumerate() {
        if matches!(c, ' ' | '\t' | '\r' | '\n') { continue; }
        if c == '=' {
            padding += 1;
            continue;
        }
        if padding > 0 {
            return syntax(format!("{name}: character {} is {} after the = padding, which must end the text", i + 1, show(c)));
        }
        match table.iter().position(|t| *t as char == c) {
            Some(v) => sextets.push(v as u32),
            None => {
                let hint = if other.contains(c) {
                    if url { " (that is the standard alphabet; use encoding.from_base64)" } else { " (that is the URL-safe alphabet; use encoding.from_base64_url)" }
                } else { "" };
                return syntax(format!("{name}: character {} is {} which is not in the base64 alphabet{hint}", i + 1, show(c)));
            }
        }
    }
    if sextets.len() % 4 == 1 {
        return syntax(format!("{name}: {} base64 characters is not a valid length (one is left over)", sextets.len()));
    }
    if padding > 2 || (padding > 0 && (sextets.len() + padding) % 4 != 0) {
        return syntax(format!("{name}: the = padding is wrong: {padding} after {} characters", sextets.len()));
    }
    let mut bytes = Vec::new();
    for group in sextets.chunks(4) {
        let mut n = 0u32;
        for k in 0..4 { n = n << 6 | group.get(k).copied().unwrap_or(0); }
        bytes.push((n >> 16) as u8);
        if group.len() > 2 { bytes.push((n >> 8) as u8); }
        if group.len() > 3 { bytes.push(n as u8); }
    }
    Ok(bytes)
}
