//! `takepkg rs.regex;` - regular expressions for `std.regex`, by Rust's
//! `regex` crate.
//!
//! The crate runs automata rather than backtracking, so matching takes time
//! in proportion to the text for every pattern: no pattern, however written,
//! can hang a program (no "catastrophic backtracking"). The price is that
//! backreferences and lookaround are not supported, and say so when used.
//!
//! Positions are characters (grapheme clusters), the unit of `len` and
//! `slice`. Patterns work on Unicode code points, as in other languages, so a
//! match can begin or end inside a character, such as an accent matched on
//! its own; its start is then the character it begins in, and its end the one
//! after the character it ends in, so `slice(start, end)` covers it.
//!
//! Matches come back raw, as `[text, start, end, groups, names]`, and
//! `std/regex.mars` makes them `Match` instances.

use std::cell::RefCell;
use std::rc::Rc;

use indexmap::IndexMap;
use regex::{Captures, Regex, RegexBuilder};
use unicode_segmentation::{GraphemeIndices, UnicodeSegmentation};

use crate::value::*;

/// Compiled patterns kept for reuse; the oldest goes first when it is full.
const CACHE_SIZE: usize = 64;

thread_local! {
    /// Keyed by the pattern and whether it must match the whole text.
    static CACHE: RefCell<IndexMap<(Rc<str>, bool), Rc<Regex>>> = RefCell::new(IndexMap::new());
}

pub fn package() -> Value {
    NativePackage::new("rs.regex")
        .function("check", 1, |args| { compiled(text(&args[0])?, false)?; Ok(Value::Null) })
        .function("escape", 1, |args| Ok(Value::str(&regex::escape(text(&args[0])?))))
        .function("contains", 2, |args| Ok(Value::Bool(compiled(text(&args[0])?, false)?.is_match(text(&args[1])?))))
        .function("matches", 2, |args| Ok(Value::Bool(compiled(text(&args[0])?, true)?.is_match(text(&args[1])?))))
        .function("find", 2, |args| first(&args[0], &args[1], false))
        .function("find_whole", 2, |args| first(&args[0], &args[1], true))
        .function("find_all", 2, |args| {
            let pattern = compiled(text(&args[0])?, false)?;
            let haystack = text(&args[1])?;
            let mut places = Places::new(haystack);
            Ok(Value::array(pattern.captures_iter(haystack).map(|caps| raw(&pattern, &caps, &mut places)).collect()))
        })
        // The text with the first match, or every match, replaced; $1, ${1},
        // and ${name} in the replacement stand for groups, and $$ for $.
        .function("replace", 4, |args| {
            let pattern = compiled(text(&args[0])?, false)?;
            let limit = match &args[3] { Value::Bool(true) => 0, _ => 1 };
            Ok(Value::str(&pattern.replacen(text(&args[1])?, limit, text(&args[2])?)))
        })
        .function("split", 2, |args| {
            let pattern = compiled(text(&args[0])?, false)?;
            Ok(Value::array(pattern.split(text(&args[1])?).map(Value::str).collect()))
        })
        // For replace_with: [the n + 1 pieces of text around the matches, the n matches].
        .function("pieces", 2, |args| {
            let pattern = compiled(text(&args[0])?, false)?;
            let haystack = text(&args[1])?;
            let mut places = Places::new(haystack);
            let (mut gaps, mut matches, mut after) = (Vec::new(), Vec::new(), 0);
            for caps in pattern.captures_iter(haystack) {
                let whole = caps.get(0).unwrap();
                gaps.push(Value::str(&haystack[after..whole.start()]));
                matches.push(raw(&pattern, &caps, &mut places));
                after = whole.end();
            }
            gaps.push(Value::str(&haystack[after..]));
            Ok(Value::array(vec![Value::array(gaps), Value::array(matches)]))
        })
        .build()
}

fn text<'a>(value: &'a Value) -> RResult<&'a str> {
    match value {
        Value::Str(text) => Ok(text),
        other => type_err(format!("std.regex expects strings, got {}", other.type_name())),
    }
}

/// The first match, or the match of the whole text, raw; null when none.
fn first(pattern: &Value, haystack: &Value, whole: bool) -> RResult<Value> {
    let pattern = compiled(text(pattern)?, whole)?;
    let haystack = text(haystack)?;
    Ok(match pattern.captures(haystack) {
        Some(caps) => raw(&pattern, &caps, &mut Places::new(haystack)),
        None => Value::Null,
    })
}

/// `[text, start, end, groups, names]` for one match.
fn raw(pattern: &Regex, caps: &Captures, places: &mut Places) -> Value {
    let whole = caps.get(0).unwrap();
    let start = places.at(whole.start(), false);
    let end = places.at(whole.end(), true);
    let group = |i: usize| caps.get(i).map_or(Value::Null, |g| Value::str(g.as_str()));
    let groups = (1..caps.len()).map(group).collect();
    let mut names = IndexMap::new();
    for (i, name) in pattern.capture_names().enumerate() {
        if let Some(name) = name {
            let key = Value::str(name);
            names.insert(Key::of(&key), (key, group(i)));
        }
    }
    Value::array(vec![
        Value::str(whole.as_str()),
        Value::Int(start as i64),
        Value::Int(end as i64),
        Value::array(groups),
        Value::new_map(names, Meta::default()),
    ])
}

/// Character positions of byte offsets that never go backwards, found in
/// one pass over the text however many matches there are.
struct Places<'a> {
    text: &'a str,
    characters: std::iter::Peekable<GraphemeIndices<'a>>,
    /// Characters that start before the last offset asked about.
    before: usize,
}

impl<'a> Places<'a> {
    fn new(text: &'a str) -> Self {
        Places { text, characters: text.grapheme_indices(true).peekable(), before: 0 }
    }

    /// The character at a byte offset; inside a character, the one it is in,
    /// or with `after`, the next one.
    fn at(&mut self, byte: usize, after: bool) -> usize {
        while self.characters.peek().is_some_and(|&(start, _)| start < byte) {
            self.characters.next();
            self.before += 1;
        }
        let on_boundary = byte == self.text.len() || self.characters.peek().is_some_and(|&(start, _)| start == byte);
        if on_boundary || after { self.before } else { self.before - 1 }
    }
}

/// The pattern compiled, from the cache when it was used lately. `whole`
/// anchors it to the start and end of the text.
fn compiled(pattern: &str, whole: bool) -> RResult<Rc<Regex>> {
    let key = (Rc::from(pattern), whole);
    if let Some(found) = CACHE.with(|cache| cache.borrow().get(&key).cloned()) {
        return Ok(found);
    }
    let regex = if whole {
        // Checked alone first, so errors show the pattern as written. A
        // comment in a pattern under (?x) runs to the end of its line and
        // would swallow the closing ")\z", so then they go on a new line,
        // where (?x) ignores the line break.
        compiled(pattern, false)?;
        build(&format!(r"\A(?:{pattern})\z"), pattern)
            .or_else(|_| build(&format!("\\A(?:{pattern}\n)\\z"), pattern))?
    } else {
        build(pattern, pattern)?
    };
    let regex = Rc::new(regex);
    CACHE.with(|cache| {
        let mut cache = cache.borrow_mut();
        if cache.len() >= CACHE_SIZE { cache.shift_remove_index(0); }
        cache.insert(key, regex.clone());
    });
    Ok(regex)
}

fn build(source: &str, pattern: &str) -> RResult<Regex> {
    RegexBuilder::new(source).build().or_else(|error| match error {
        regex::Error::CompiledTooBig(limit) => range_err(format!(
            "regex {} is too large: compiled, it would pass the limit of {} MB", quoted(pattern), limit >> 20
        )),
        other => err(ErrorKind::SyntaxError, describe(pattern, &other.to_string())),
    })
}

/// One line saying what is wrong with a pattern, and where: the regex
/// crate's own message draws the pattern over several lines.
fn describe(pattern: &str, fallback: &str) -> String {
    let (what, at) = match regex_syntax::parse(pattern) {
        Err(regex_syntax::Error::Parse(e)) => (e.kind().to_string(), e.span().start),
        Err(regex_syntax::Error::Translate(e)) => (e.kind().to_string(), e.span().start),
        _ => return format!("regex {}: {}", quoted(pattern), fallback.lines().last().unwrap_or(fallback).trim_start_matches("error: ")),
    };
    let place = if at.line > 1 { format!("line {}, character {}", at.line, at.column) } else { format!("character {}", at.column) };
    format!("regex {} at {place}: {what}", quoted(pattern))
}

/// The pattern in quotes for a message, as written (backslashes single, as in
/// r"..."), on one line, and shortened when long.
fn quoted(pattern: &str) -> String {
    let shown: String = pattern.chars().take(60).collect();
    let more = if shown.len() < pattern.len() { "..." } else { "" };
    let line: String = shown.chars().map(|c| match c {
        '\n' => "\\n".to_string(),
        '\r' => "\\r".to_string(),
        '\t' => "\\t".to_string(),
        c if c.is_control() => c.escape_unicode().to_string(),
        c => c.to_string(),
    }).collect();
    format!("\"{line}{more}\"")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn places_count_characters_and_cover_split_ones() {
        // "e" + combining acute is one character of three bytes.
        let text = "ae\u{301}b";
        let mut places = Places::new(text);
        assert_eq!(places.at(0, false), 0);
        assert_eq!(places.at(1, false), 1);
        // Inside the second character: it starts there and ends after it.
        assert_eq!(places.at(2, false), 1);
        assert_eq!(places.at(2, true), 2);
        assert_eq!(places.at(4, true), 2);
        assert_eq!(places.at(5, true), 3);
    }

    #[test]
    fn whole_matches_survive_comments_under_x() {
        let regex = compiled("(?x) \\d+ # digits", true).unwrap();
        assert!(regex.is_match("123"));
        assert!(!regex.is_match("123a"));
        assert!(compiled("a|ab", true).unwrap().is_match("ab"));
    }

    #[test]
    fn errors_are_one_line_and_say_where() {
        let error = compiled("ab(c", false).unwrap_err();
        assert_eq!(error.message, "regex \"ab(c\" at character 3: unclosed group");
    }
}
