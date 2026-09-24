//! `takepkg rs.config;` - configuration formats for `std.file.env`,
//! `std.file.ini`, and `std.file.toml`: each read from text into maps and
//! written back. `source` names where text came from, such as a file, so
//! every error can say where it is.

use std::collections::HashSet;

use indexmap::IndexMap;

use crate::value::*;

fn text<'a>(value: &'a Value, what: &str) -> RResult<&'a str> {
    match value {
        Value::Str(text) => Ok(text),
        other => type_err(format!("{what} expects a string, got {}", other.type_name())),
    }
}

fn place(source: &str, format: &str, line: usize) -> String {
    if source.is_empty() { format!("{format} line {line}") } else { format!("{source}: {format} line {line}") }
}

fn syntax<T>(source: &str, format: &str, line: usize, message: impl std::fmt::Display) -> RResult<T> {
    err(ErrorKind::SyntaxError, format!("{}: {message}", place(source, format, line)))
}

fn new_map(entries: IndexMap<String, Value>) -> Value {
    let mut items = IndexMap::new();
    for (key, value) in entries {
        let key = Value::str(&key);
        items.insert(Key::of(&key), (key, value));
    }
    Value::new_map(items, Meta::default())
}

/// The entries of a map with string keys, in order.
fn entries(value: &Value, what: &str) -> RResult<Vec<(String, Value)>> {
    let Value::Map(map) = value else { return type_err(format!("{what} expects a map, got {}", value.type_name())) };
    map.items.borrow().values().map(|(key, value)| match key {
        Value::Str(key) => Ok((key.to_string(), value.clone())),
        other => type_err(format!("{what} needs string keys; this map has the key {} ({})", to_display_string(other), other.type_name())),
    }).collect()
}

/// A plain value as text: strings as they are, numbers and booleans as `out`
/// prints them.
fn scalar(value: &Value, what: &str) -> RResult<String> {
    match value {
        Value::Str(text) => Ok(text.to_string()),
        Value::Int(_) | Value::Long(_) | Value::Float(_) | Value::Bool(_) => Ok(to_display_string(value)),
        Value::Date(_) | Value::DateTime(_) | Value::Duration(_) => Ok(to_display_string(value)),
        other => type_err(format!("{what} values are text, numbers, or booleans, not {} {}", article(other.type_name()), other.type_name())),
    }
}

fn article(kind: &str) -> &'static str {
    if kind.starts_with(['a', 'e', 'i', 'o', 'u']) { "an" } else { "a" }
}

fn is_name(key: &str) -> bool {
    key.chars().next().is_some_and(|c| c.is_ascii_alphabetic() || c == '_')
        && key.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
}

// ----- .env -----

/// `KEY=value` lines, as Docker, Compose, and python-dotenv read them: blank
/// lines and `#` comments are skipped, `export ` before a key is allowed, a
/// double-quoted value takes `\n`, `\t`, `\"`, and `\\` escapes and may span
/// lines, a single-quoted one is taken exactly, and an unquoted one ends at a
/// ` #` comment. Values are not expanded: `$HOME` stays `$HOME`.
fn env_parse(text: &str, source: &str) -> RResult<Value> {
    let chars: Vec<char> = text.strip_prefix('\u{feff}').unwrap_or(text).chars().collect();
    let mut result = IndexMap::new();
    let (mut at, mut line) = (0, 1);
    while at < chars.len() {
        // One entry: skip spaces, then a comment, a blank line, or KEY=value.
        while at < chars.len() && matches!(chars[at], ' ' | '\t') { at += 1; }
        if at >= chars.len() { break; }
        match chars[at] {
            '\n' => { at += 1; line += 1; continue; }
            '\r' => { at += 1; continue; }
            '#' => {
                while at < chars.len() && chars[at] != '\n' { at += 1; }
                continue;
            }
            _ => {}
        }
        let start_line = line;
        let rest: String = chars[at..].iter().take_while(|&&c| c != '\n').collect();
        if let Some(after) = rest.strip_prefix("export").filter(|a| a.starts_with([' ', '\t'])) {
            at += rest.len() - after.len();
            while matches!(chars.get(at), Some(' ' | '\t')) { at += 1; }
        }
        let key_start = at;
        while at < chars.len() && !matches!(chars[at], '=' | '\n') { at += 1; }
        if chars.get(at) != Some(&'=') {
            return syntax(source, "env", start_line, "expected KEY=value");
        }
        let key: String = chars[key_start..at].iter().collect::<String>().trim().to_string();
        if !is_name(&key) {
            return syntax(source, "env", start_line, format!("\"{key}\" is not a variable name: letters, digits, and _, not starting with a digit"));
        }
        at += 1;
        while matches!(chars.get(at), Some(' ' | '\t')) { at += 1; }
        let value = match chars.get(at) {
            Some(&quote @ ('"' | '\'')) => {
                at += 1;
                let mut value = String::new();
                loop {
                    match chars.get(at) {
                        None => return syntax(source, "env", start_line, format!("the value of {key} opens a {quote} quote that is never closed")),
                        Some(&c) if c == quote => { at += 1; break; }
                        Some('\\') if quote == '"' => {
                            at += 1;
                            match chars.get(at) {
                                Some('n') => value.push('\n'),
                                Some('r') => value.push('\r'),
                                Some('t') => value.push('\t'),
                                Some('"') => value.push('"'),
                                Some('\\') => value.push('\\'),
                                Some('$') => value.push('$'),
                                Some(&other) => { value.push('\\'); value.push(other); if other == '\n' { line += 1; } }
                                None => continue,
                            }
                            at += 1;
                        }
                        Some(&c) => { if c == '\n' { line += 1; } value.push(c); at += 1; }
                    }
                }
                // After the closing quote only spaces and a comment may follow.
                while matches!(chars.get(at), Some(' ' | '\t' | '\r')) { at += 1; }
                match chars.get(at) {
                    None | Some('\n') => {}
                    Some('#') => { while at < chars.len() && chars[at] != '\n' { at += 1; } }
                    Some(other) => return syntax(source, "env", line, format!("'{other}' after the closing quote of {key}")),
                }
                value
            }
            _ => {
                let start = at;
                while at < chars.len() && chars[at] != '\n' {
                    // " #" starts a comment; a # inside a word does not.
                    if chars[at] == '#' && at > start && matches!(chars[at - 1], ' ' | '\t') { break; }
                    at += 1;
                }
                let value: String = chars[start..at].iter().collect();
                while at < chars.len() && chars[at] != '\n' { at += 1; }
                value.trim_end().to_string()
            }
        };
        // A key given twice keeps its first position and its last value, as
        // JSON objects do.
        result.insert(key, Value::str(&value));
    }
    Ok(new_map(result))
}

fn env_format(value: &Value) -> RResult<Value> {
    let mut out = String::new();
    for (key, value) in entries(value, "env.format")? {
        if !is_name(&key) {
            return range_err(format!("\"{key}\" is not a variable name: letters, digits, and _, not starting with a digit"));
        }
        let text = scalar(&value, "env")?;
        out.push_str(&key);
        out.push('=');
        let plain = text.chars().all(|c| c.is_ascii_alphanumeric() || "_-./:@,+%".contains(c));
        if plain {
            out.push_str(&text);
        } else if !text.contains(['\'', '\n', '\r']) {
            // Single quotes are taken exactly by every reader, including
            // Docker and a shell, so $ and # inside stay as they are.
            out.push('\'');
            out.push_str(&text);
            out.push('\'');
        } else {
            out.push('"');
            for c in text.chars() {
                match c {
                    '"' => out.push_str("\\\""),
                    '\\' => out.push_str("\\\\"),
                    '\n' => out.push_str("\\n"),
                    '\r' => out.push_str("\\r"),
                    '\t' => out.push_str("\\t"),
                    c => out.push(c),
                }
            }
            out.push('"');
        }
        out.push('\n');
    }
    Ok(Value::str(&out))
}

// ----- INI -----

/// `[section]` headers and `key = value` or `key: value` lines. Keys before
/// the first header are top-level keys; each section is a map inside the
/// result. Lines starting with `;` or `#` are comments. Values are text, with
/// the spaces around them removed; there is no quoting.
fn ini_parse(text: &str, source: &str) -> RResult<Value> {
    let text = text.strip_prefix('\u{feff}').unwrap_or(text);
    let mut top: IndexMap<String, Value> = IndexMap::new();
    let mut sections: IndexMap<String, IndexMap<String, Value>> = IndexMap::new();
    let mut current: Option<String> = None;
    for (index, raw) in text.lines().enumerate() {
        let line = index + 1;
        let trimmed = raw.trim();
        if trimmed.is_empty() || trimmed.starts_with([';', '#']) { continue; }
        if let Some(inner) = trimmed.strip_prefix('[') {
            let Some(name) = inner.strip_suffix(']') else {
                return syntax(source, "INI", line, "a section header ends with ]");
            };
            let name = name.trim().to_string();
            if name.is_empty() { return syntax(source, "INI", line, "a section needs a name"); }
            if sections.contains_key(&name) { return syntax(source, "INI", line, format!("the section [{name}] appears twice")); }
            if top.contains_key(&name) { return syntax(source, "INI", line, format!("[{name}] has the same name as a key above it")); }
            sections.insert(name.clone(), IndexMap::new());
            current = Some(name);
            continue;
        }
        let Some(split) = trimmed.find(['=', ':']) else {
            return syntax(source, "INI", line, "expected key = value, [section], or a comment");
        };
        let key = trimmed[..split].trim().to_string();
        let value = trimmed[split + 1..].trim();
        if key.is_empty() { return syntax(source, "INI", line, "a key is missing before the ="); }
        let table = match &current { Some(name) => sections.get_mut(name).unwrap(), None => &mut top };
        if table.contains_key(&key) {
            let within = current.as_ref().map(|s| format!(" in [{s}]")).unwrap_or_default();
            return syntax(source, "INI", line, format!("the key {key} appears twice{within}"));
        }
        table.insert(key, Value::str(value));
    }
    for (name, table) in sections { top.insert(name, new_map(table)); }
    Ok(new_map(top))
}

fn ini_key(key: &str) -> RResult<()> {
    if key.is_empty() || key.contains(['=', ':', '\n', '\r', '[', ']']) || key.starts_with([';', '#']) || key.trim() != key {
        return range_err(format!("\"{key}\" cannot be an INI key: it needs a name without =, :, brackets, line breaks, or spaces at the ends"));
    }
    Ok(())
}

fn ini_value(value: &Value) -> RResult<String> {
    let text = scalar(value, "INI")?;
    if text.contains(['\n', '\r']) { return range_err("an INI value cannot hold a line break"); }
    Ok(text)
}

fn ini_format(value: &Value) -> RResult<Value> {
    let mut out = String::new();
    let mut sections = Vec::new();
    for (key, value) in entries(value, "ini.format")? {
        ini_key(&key)?;
        if let Value::Map(_) = value { sections.push((key, value)); continue; }
        out.push_str(&format!("{key} = {}\n", ini_value(&value)?));
    }
    for (name, table) in sections {
        if !out.is_empty() { out.push('\n'); }
        out.push_str(&format!("[{name}]\n"));
        for (key, value) in entries(&table, "ini.format")? {
            ini_key(&key)?;
            if let Value::Map(_) = value { return type_err(format!("INI has one level of sections; [{name}] holds the map {key}")); }
            out.push_str(&format!("{key} = {}\n", ini_value(&value)?));
        }
    }
    Ok(Value::str(&out))
}

// ----- TOML -----

/// A TOML document as a map; dates become `date` and `datetime` values
/// (see `from_toml_datetime`).
fn toml_parse(text: &str, source: &str) -> RResult<Value> {
    match text.parse::<toml::Table>() {
        Ok(table) => Ok(from_toml(toml::Value::Table(table))),
        Err(error) => {
            let mut at = error.span().map(|span| span.start).unwrap_or(0).min(text.len());
            while !text.is_char_boundary(at) { at -= 1; }
            let before = &text[..at];
            let line = before.matches('\n').count() + 1;
            let column = before.rsplit('\n').next().unwrap_or("").chars().count() + 1;
            let message = error.message().trim().trim_end_matches('.').to_string();
            let from = if source.is_empty() { String::new() } else { format!("{source}: ") };
            err(ErrorKind::SyntaxError, format!("{from}TOML line {line}, column {column}: {message}"))
        }
    }
}

fn from_toml(value: toml::Value) -> Value {
    match value {
        toml::Value::String(text) => Value::str(&text),
        toml::Value::Integer(n) => if i32::try_from(n).is_ok() { Value::Int(n) } else { Value::Long(n) },
        toml::Value::Float(f) => Value::Float(f),
        toml::Value::Boolean(b) => Value::Bool(b),
        toml::Value::Datetime(when) => from_toml_datetime(when),
        toml::Value::Array(items) => Value::array(items.into_iter().map(from_toml).collect()),
        toml::Value::Table(table) => new_map(table.into_iter().map(|(k, v)| (k, from_toml(v))).collect()),
    }
}

/// A TOML date becomes a date, and a date and time with an offset a
/// datetime at that offset. A local date and time, or a time alone, names no
/// moment, so it stays text.
fn from_toml_datetime(when: toml::value::Datetime) -> Value {
    match (&when.date, &when.time, &when.offset) {
        (Some(date), None, None) => match jiff::civil::Date::new(date.year as i16, date.month as i8, date.day as i8) {
            Ok(date) => crate::date::date_value(date),
            Err(_) => Value::str(&when.to_string()),
        },
        (Some(_), Some(_), Some(_)) => match crate::date::make_datetime(&[Value::str(&when.to_string())]) {
            Ok(moment) => moment,
            Err(_) => Value::str(&when.to_string()),
        },
        _ => Value::str(&when.to_string()),
    }
}

fn to_toml(value: &Value, open: &mut HashSet<usize>) -> RResult<toml::Value> {
    Ok(match value {
        Value::Date(date) if date.year() < 0 => return type_err(format!("TOML dates have four-digit years; {} cannot be written", crate::date::show_date(date))),
        Value::Date(date) => toml::Value::Datetime(toml::value::Datetime {
            date: Some(toml::value::Date { year: date.year() as u16, month: date.month() as u8, day: date.day() as u8 }),
            time: None,
            offset: None,
        }),
        // TOML has no zone names, so a datetime is written with its offset.
        Value::DateTime(moment) => {
            let text = format!("{}{}", moment.datetime(), crate::date::show_offset(moment.offset()));
            match text.parse::<toml::value::Datetime>() {
                Ok(when) => toml::Value::Datetime(when),
                Err(_) => return type_err(format!("the datetime {} cannot be written as TOML", crate::date::show_datetime(moment))),
            }
        }
        Value::Duration(_) => return type_err("TOML has no durations; write the number of seconds or the text instead"),
        Value::Str(text) => toml::Value::String(text.to_string()),
        Value::Int(n) | Value::Long(n) => toml::Value::Integer(*n),
        Value::Float(f) => toml::Value::Float(*f),
        Value::Bool(b) => toml::Value::Boolean(*b),
        Value::Null => return type_err("TOML has no null; leave the key out instead"),
        Value::Array(array) => {
            let id = value.object_id().expect("arrays have an identity");
            if !open.insert(id) { return type_err("this array contains itself, so it cannot be written as TOML"); }
            let items = array.items.borrow().clone();
            let converted = items.iter().map(|item| to_toml(item, open)).collect::<RResult<Vec<_>>>()?;
            open.remove(&id);
            toml::Value::Array(converted)
        }
        Value::Map(_) => {
            let id = value.object_id().expect("maps have an identity");
            if !open.insert(id) { return type_err("this map contains itself, so it cannot be written as TOML"); }
            let mut table = toml::Table::new();
            for (key, item) in entries(value, "toml.format")? { table.insert(key, to_toml(&item, open)?); }
            open.remove(&id);
            toml::Value::Table(table)
        }
        other => return type_err(format!("{} {} cannot be written as TOML", article(other.type_name()), other.type_name())),
    })
}

fn toml_format(value: &Value) -> RResult<Value> {
    if !matches!(value, Value::Map(_)) {
        return type_err(format!("a TOML document is a map, not {} {}", article(value.type_name()), value.type_name()));
    }
    let toml::Value::Table(table) = to_toml(value, &mut HashSet::new())? else { unreachable!("a map becomes a table") };
    match toml::to_string(&table) {
        Ok(text) => Ok(Value::str(&text)),
        Err(error) => type_err(format!("this value cannot be written as TOML: {error}")),
    }
}

pub fn package() -> Value {
    NativePackage::new("rs.config")
        .function("env_parse", 2, |a| env_parse(text(&a[0], "env.parse")?, text(&a[1], "env.parse")?))
        .function("env_format", 1, |a| env_format(&a[0]))
        .function("ini_parse", 2, |a| ini_parse(text(&a[0], "ini.parse")?, text(&a[1], "ini.parse")?))
        .function("ini_format", 1, |a| ini_format(&a[0]))
        .function("toml_parse", 2, |a| toml_parse(text(&a[0], "toml.parse")?, text(&a[1], "toml.parse")?))
        .function("toml_format", 1, |a| toml_format(&a[0]))
        .build()
}
