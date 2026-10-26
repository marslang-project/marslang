use crate::ast::Expr;

#[derive(Clone, Debug)]
enum Token {
    Name(String), Number(String), String(String), Template(String), Op(String),
    /// `func(params) => body`, and whether it is `async func`.
    Lambda(String, bool),
    /// `match (subject){ arms }`: the text of the subject and of the arms.
    Match(String, String),
    End,
}

pub fn parse(text: &str) -> Result<Expr, String> {
    let mut tokens = Vec::new();
    let mut chars = text.char_indices().peekable();
    while let Some((start, ch)) = chars.next() {
        if ch.is_whitespace() { continue; }
        if ch.is_ascii_alphabetic() || ch == '_' {
            let mut word = ch.to_string();
            while let Some((_, c)) = chars.peek() {
                if !c.is_ascii_alphanumeric() && *c != '_' { break; }
                word.push(chars.next().unwrap().1);
            }
            let member = matches!(tokens.last(), Some(Token::Op(op)) if op == ".");
            // `async func(...) => ...` is an async anonymous function. Before a
            // dot, async is the name std.async is imported under.
            let mut asynchronous = false;
            if word == "async" && !member {
                let mut ahead = chars.clone();
                while matches!(ahead.peek(), Some((_, c)) if c.is_whitespace()) { ahead.next(); }
                let mut next = String::new();
                while let Some((_, c)) = ahead.peek() {
                    if !c.is_ascii_alphanumeric() && *c != '_' { break; }
                    next.push(ahead.next().unwrap().1);
                }
                if next == "func" {
                    chars = ahead;
                    word = next;
                    asynchronous = true;
                } else if !matches!(chars.peek(), Some((_, '.'))) {
                    return Err("'async' is a keyword: it marks a function that runs as a task, as in \
                                async func load(string url){ ... } or async func(int x) => x + 1".into());
                }
            }
            if word == "await" && !member {
                tokens.push(Token::Op(word));
                continue;
            }
            if word == "func" {
                // func(type name, ...) => body: an anonymous function. Its body is
                // one expression; a longer one is a named func inside the block.
                let mut ahead = chars.clone();
                while matches!(ahead.peek(), Some((_, c)) if c.is_whitespace()) { ahead.next(); }
                if let Some((open, '(')) = ahead.peek().copied() {
                    ahead.next();
                    let (mut depth, mut close, mut quote, mut escaped) = (1usize, None, None, false);
                    for (at, c) in ahead.by_ref() {
                        if let Some(q) = quote {
                            if escaped { escaped = false; } else if c == '\\' { escaped = true; } else if c == q { quote = None; }
                            continue;
                        }
                        match c {
                            '"' | '\'' => quote = Some(c),
                            '(' => depth += 1,
                            ')' => { depth -= 1; if depth == 0 { close = Some(at); break; } }
                            _ => {}
                        }
                    }
                    let close = close.ok_or("an anonymous function is missing the ')' after its parameters")?;
                    while matches!(ahead.peek(), Some((_, c)) if c.is_whitespace()) { ahead.next(); }
                    let arrow = matches!(ahead.next(), Some((_, '='))) && matches!(ahead.next(), Some((_, '>')));
                    if !arrow {
                        return Err("an anonymous function is written func(type name) => expression; \
                                    for a longer body, declare a named func inside the block".into());
                    }
                    chars = ahead;
                    tokens.push(Token::Lambda(text[open + 1..close].to_string(), asynchronous));
                    continue;
                }
                if asynchronous {
                    return Err("an async anonymous function is written async func(type name) => expression".into());
                }
            }
            // A member may be named match, as in obj.match, but nothing else.
            if word == "match" && !matches!(tokens.last(), Some(Token::Op(op)) if op == ".") {
                let from = chars.peek().map_or(text.len(), |(at, _)| *at);
                let (subject, body, end) = match_parts(text, from).ok_or(
                    "'match' is a keyword: a match expression is written match (value){ pattern => result; ... }")?;
                while chars.peek().is_some_and(|(at, _)| *at < end) { chars.next(); }
                tokens.push(Token::Match(subject, body));
                continue;
            }
            if let Some((_, quote @ ('"' | '\''))) = chars.peek().copied() {
                if word == "f" {
                    // f"...": an f-string, kept undecoded until its braces are found.
                    let open = chars.next().unwrap().0;
                    let mut escaped = false; let mut end = None;
                    for (offset, c) in chars.by_ref() {
                        if escaped { escaped = false; }
                        else if c == '\\' { escaped = true; }
                        else if c == quote { end = Some(offset); break; }
                    }
                    let end = end.ok_or("unterminated f-string")?;
                    tokens.push(Token::Template(text[open + 1..end].to_string()));
                    continue;
                }
                if matches!(word.as_str(), "rf" | "fr") {
                    return Err(format!("{word}\"...\" is not a kind of string: an f-string cannot also be raw"));
                }
            }
            tokens.push(if matches!(word.as_str(), "and" | "or" | "not") {
                Token::Op(word)
            } else { Token::Name(word) });
        } else if ch.is_ascii_digit() {
            let mut value = ch.to_string();
            while let Some((_, c)) = chars.peek() {
                if !c.is_ascii_digit() { break; }
                value.push(chars.next().unwrap().1);
            }
            if chars.peek().map(|(_, c)| *c) == Some('.') {
                let mut lookahead = chars.clone(); lookahead.next();
                if lookahead.peek().map(|(_, c)| c.is_ascii_digit()).unwrap_or(false) {
                    value.push(chars.next().unwrap().1);
                    while let Some((_, c)) = chars.peek() {
                        if !c.is_ascii_digit() { break; }
                        value.push(chars.next().unwrap().1);
                    }
                }
            }
            if matches!(chars.peek().map(|(_, c)| *c), Some('e' | 'E')) {
                value.push(chars.next().unwrap().1);
                if matches!(chars.peek().map(|(_, c)| *c), Some('+' | '-')) {
                    value.push(chars.next().unwrap().1);
                }
                let before = value.len();
                while let Some((_, c)) = chars.peek() {
                    if !c.is_ascii_digit() { break; }
                    value.push(chars.next().unwrap().1);
                }
                if value.len() == before { return Err("number exponent needs digits".into()); }
            }
            tokens.push(Token::Number(value));
        } else if ch == '\'' || ch == '"' {
            let mut escaped = false; let mut end = None;
            for (offset, c) in chars.by_ref() {
                if escaped { escaped = false; }
                else if c == '\\' { escaped = true; }
                else if c == ch { end = Some(offset + c.len_utf8()); break; }
            }
            let end = end.ok_or("unterminated string")?;
            tokens.push(Token::String(decode_string(&text[start + 1..end - 1])?));
        } else {
            let mut op = ch.to_string();
            if let Some((_, next)) = chars.peek() {
                let combined = format!("{ch}{next}");
                if matches!(combined.as_str(), "==" | "!=" | "<=" | ">=" | "**") {
                    op = combined; chars.next();
                }
            }
            if !matches!(op.as_str(), "+" | "-" | "*" | "/" | "%" | "**" | "=" | "==" | "!=" | "<" | ">" | "<=" | ">=" | "(" | ")" | "[" | "]" | "," | ".") {
                return Err(format!("unsupported expression token '{op}' at column {}", start + 1));
            }
            tokens.push(Token::Op(op));
        }
    }
    tokens.push(Token::End);
    let mut parser = Parser { tokens, pos: 0, depth: 0 };
    let value = parser.expr(0)?;
    if !matches!(parser.tokens[parser.pos], Token::End) {
        return Err(format!("unexpected expression token {:?}", parser.tokens[parser.pos]));
    }
    Ok(value)
}

/// Deeper nesting in one expression is refused with an error rather than
/// risking the stack; no written program comes near it.
const MAX_NESTING: usize = 1000;
/// Operators in a row, such as a + b + c: each one deepens the tree the
/// interpreter walks, so a machine-made expression cannot overflow it.
const MAX_CHAIN: usize = 10_000;

struct Parser { tokens: Vec<Token>, pos: usize, depth: usize }
impl Parser {
    fn take(&mut self) -> Token {
        let token = self.tokens[self.pos].clone();
        if !matches!(token, Token::End) { self.pos += 1; }
        token
    }
    fn is(&self, op: &str) -> bool { matches!(&self.tokens[self.pos], Token::Op(s) if s == op) }
    fn expect(&mut self, op: &str) -> Result<(), String> {
        if !self.is(op) { return Err(format!("expected '{op}' in expression")); }
        self.pos += 1; Ok(())
    }
    fn expr(&mut self, min: u8) -> Result<Expr, String> {
        if self.depth == MAX_NESTING {
            return Err(format!("this expression is nested more than {MAX_NESTING} levels deep"));
        }
        self.depth += 1;
        let result = self.expr_at(min);
        self.depth -= 1;
        result
    }

    fn expr_at(&mut self, min: u8) -> Result<Expr, String> {
        let mut chained = 0;
        let mut left = match self.take() {
            Token::Name(s) => match s.as_str() {
                "true" => Expr::Bool(true), "false" | "fasle" => Expr::Bool(false),
                "null" => Expr::Null, _ => Expr::Ident(s),
            },
            Token::Number(s) => Expr::Number(s), Token::String(s) => Expr::String(s),
            Token::Template(body) => template(&body)?,
            Token::Lambda(params, asynchronous) => {
                let body = self.expr(0)?;
                let mut decl = crate::parser::lambda(&params, body)?;
                decl.is_async = asynchronous;
                Expr::Lambda(std::sync::Arc::new(decl))
            }
            Token::Match(subject, body) => crate::parser::match_expression(&subject, &body)?,
            Token::Op(s) if s == "(" => { let v = self.expr(0)?; self.expect(")")?; v }
            Token::Op(op) if matches!(op.as_str(), "+" | "-" | "not") => {
                let value = self.expr(if op == "not" { 3 } else { 6 })?;
                Expr::Unary { op, value: Box::new(value) }
            }
            // Tighter than every binary operator: await a + await b adds two results.
            Token::Op(op) if op == "await" => {
                if matches!(self.tokens[self.pos], Token::End) {
                    return Err("await needs a task to wait for, as in await load(url)".into());
                }
                Expr::Await(Box::new(self.expr(7)?))
            }
            t => return Err(format!("expected expression, got {t:?}")),
        };
        loop {
            if self.is(".") {
                self.pos += 1;
                let Token::Name(field) = self.take() else { return Err("expected member name".into()); };
                left = Expr::Member { object: Box::new(left), field }; continue;
            }
            if self.is("[") {
                self.pos += 1;
                let index = self.expr(0)?;
                self.expect("]")?;
                left = Expr::Index { object: Box::new(left), index: Box::new(index) }; continue;
            }
            if self.is("(") {
                self.pos += 1; let mut args = Vec::new();
                let slicing = matches!(&left, Expr::Member { field, .. } if field == "slice" || field == "lenslice");
                let mut named_reverse = false;
                if !self.is(")") {
                    loop {
                        let named = matches!(self.tokens.get(self.pos), Some(Token::Name(_)))
                            && matches!(self.tokens.get(self.pos + 1), Some(Token::Op(op)) if op == "=");
                        if named {
                            let Token::Name(name) = self.take() else { unreachable!() };
                            if !slicing || name != "reverse" {
                                return Err("named arguments are only supported as reverse= in slicing calls".into());
                            }
                            if args.len() != 2 || named_reverse {
                                return Err("slicing reverse= must follow exactly two positional arguments".into());
                            }
                            self.expect("=")?;
                            named_reverse = true;
                        } else if named_reverse {
                            return Err("no arguments may follow slicing reverse=".into());
                        }
                        args.push(self.expr(0)?);
                        if !self.is(",") { break; } self.pos += 1;
                    }
                }
                self.expect(")")?;
                left = Expr::Call { callee: Box::new(left), args }; continue;
            }
            let Token::Op(op) = &self.tokens[self.pos] else { break; };
            let (precedence, right_assoc) = match op.as_str() {
                "or" => (1, false), "and" => (2, false),
                "==" | "!=" | "<" | ">" | "<=" | ">=" => (3, false),
                "+" | "-" => (4, false), "*" | "/" | "%" => (5, false),
                "**" => (6, true), _ => break,
            };
            if precedence < min { break; }
            chained += 1;
            if chained > MAX_CHAIN {
                return Err(format!("this expression has more than {MAX_CHAIN} operators in a row; split it into several statements"));
            }
            let op = op.clone(); self.pos += 1;
            let right = self.expr(precedence + u8::from(!right_assoc))?;
            left = Expr::Binary { left: Box::new(left), op, right: Box::new(right) };
        }
        Ok(left)
    }
}

/// `(subject){ body }` starting at `text[from..]`, after the word `match`:
/// the subject, the body, and where the closing brace ends.
fn match_parts(text: &str, from: usize) -> Option<(String, String, usize)> {
    let rest = &text[from..];
    let open = from + rest.len() - rest.trim_start().len();
    if !text[open..].starts_with('(') { return None; }
    let close = closing(text, open, '(', ')')?;
    let after = &text[close + 1..];
    let brace = close + 1 + after.len() - after.trim_start().len();
    if !text[brace..].starts_with('{') { return None; }
    let end = closing(text, brace, '{', '}')?;
    Some((text[open + 1..close].to_string(), text[brace + 1..end].to_string(), end + 1))
}

/// Where the `close` matching the `open` at `start` is, outside strings.
fn closing(text: &str, start: usize, open: char, close: char) -> Option<usize> {
    let (mut depth, mut quote, mut escaped) = (0usize, None, false);
    for (i, ch) in text[start..].char_indices() {
        if let Some(q) = quote {
            if escaped { escaped = false; } else if ch == '\\' { escaped = true; } else if ch == q { quote = None; }
            continue;
        }
        if ch == '"' || ch == '\'' { quote = Some(ch); }
        else if ch == open { depth += 1; }
        else if ch == close {
            depth -= 1;
            if depth == 0 { return Some(start + i); }
        }
    }
    None
}

/// An f-string's parts from its text between the quotes: the pieces of text,
/// decoded like any string, and the expressions between braces, each parsed
/// on its own. `{{` and `}}` are braces.
fn template(body: &str) -> Result<Expr, String> {
    let chars: Vec<char> = body.chars().collect();
    let mut parts = Vec::new();
    // The current piece of text, still escaped.
    let mut text = String::new();
    let mut i = 0;
    while i < chars.len() {
        match chars[i] {
            '\\' => {
                text.push('\\');
                if let Some(&next) = chars.get(i + 1) { text.push(next); }
                i += 2;
            }
            '{' if chars.get(i + 1) == Some(&'{') => { text.push('{'); i += 2; }
            '}' if chars.get(i + 1) == Some(&'}') => { text.push('}'); i += 2; }
            '}' => return Err("an f-string has a '}' that closes nothing; write }} for a brace".into()),
            '{' => {
                let (source, close) = template_expression(&chars, i + 1)?;
                if source.trim().is_empty() {
                    return Err("an f-string has empty braces; put an expression in them, or write {{ for a brace".into());
                }
                if !text.is_empty() { parts.push(Expr::String(decode_string(&std::mem::take(&mut text))?)); }
                parts.push(parse(&source).map_err(|e| format!("in the f-string braces {{{}}}: {e}", source.trim()))?);
                i = close + 1;
            }
            c => { text.push(c); i += 1; }
        }
    }
    if !text.is_empty() || parts.is_empty() { parts.push(Expr::String(decode_string(&text)?)); }
    Ok(Expr::Template(parts))
}

/// The expression in braces opened just before `start`, and where its `}`
/// is. Within the braces `\"` and `\'` are quotes, since the f-string's own
/// quoting needed the backslash (and a `"""` f-string adds it to every `"`),
/// so a string can be written inside: `f"{name.replace(\"-\", \" \")}"`.
fn template_expression(chars: &[char], start: usize) -> Result<(String, usize), String> {
    let mut source = String::new();
    let mut quote: Option<char> = None;
    let mut depth = 0usize;
    let mut i = start;
    while i < chars.len() {
        let mut c = chars[i];
        if c == '\\' {
            match chars.get(i + 1) {
                Some(&q @ ('"' | '\'')) => { i += 1; c = q; }
                Some(&next) if quote.is_some() => {
                    // An escape in a string written inside the braces.
                    source.push(c);
                    source.push(next);
                    i += 2;
                    continue;
                }
                _ => {}
            }
        }
        match quote {
            Some(q) => if c == q { quote = None; },
            None => match c {
                '"' | '\'' => quote = Some(c),
                '(' | '[' => depth += 1,
                ')' | ']' => depth = depth.saturating_sub(1),
                '}' if depth == 0 => return Ok((source, i)),
                ':' if depth == 0 => return Err(
                    "f-string braces hold one expression, with no format after ':'; \
                     format numbers with std.format inside them, as in {format.fixed(price, 2)}".into()),
                _ => {}
            },
        }
        source.push(c);
        i += 1;
    }
    Err("an f-string has a '{' with no '}' to close it. A string inside the braces needs the other kind of quote, as in f\"{scores.get('ada')}\", and {{ writes a brace".into())
}

/// Decode the escapes of a quoted literal's contents: \n \r \t \b \f \v \0,
/// \xHH, \uHHHH (surrogate pairs combine), \u{H..}, and \<char> for any other char.
fn decode_string(body: &str) -> Result<String, String> {
    fn hex(chars: &mut std::iter::Peekable<std::str::Chars>, count: usize) -> Result<u32, String> {
        let mut value = 0;
        for _ in 0..count {
            let digit = chars.next().and_then(|c| c.to_digit(16)).ok_or("invalid hexadecimal escape in string")?;
            value = value * 16 + digit;
        }
        Ok(value)
    }
    let mut out = String::with_capacity(body.len());
    let mut chars = body.chars().peekable();
    let mut pending_high: Option<u32> = None;
    while let Some(ch) = chars.next() {
        if ch != '\\' {
            if pending_high.take().is_some() { out.push('\u{FFFD}'); }
            out.push(ch);
            continue;
        }
        let esc = chars.next().ok_or("unterminated escape in string")?;
        let code = match esc {
            'n' => '\n' as u32, 'r' => '\r' as u32, 't' => '\t' as u32,
            'b' => 8, 'f' => 12, 'v' => 11, '0' => 0,
            'x' => hex(&mut chars, 2)?,
            'u' if chars.peek() == Some(&'{') => {
                chars.next();
                let mut value = 0u32;
                let mut digits = 0;
                loop {
                    let c = chars.next().ok_or("unterminated \\u{...} escape in string")?;
                    if c == '}' { break; }
                    value = value.checked_mul(16).and_then(|v| v.checked_add(c.to_digit(16)?))
                        .filter(|v| *v <= 0x10FFFF).ok_or("invalid \\u{...} escape in string")?;
                    digits += 1;
                }
                if digits == 0 { return Err("invalid \\u{...} escape in string".into()); }
                value
            }
            'u' => hex(&mut chars, 4)?,
            other => other as u32,
        };
        if let Some(high) = pending_high.take() {
            if (0xDC00..0xE000).contains(&code) {
                out.push(char::from_u32(0x10000 + ((high - 0xD800) << 10) + (code - 0xDC00)).unwrap());
                continue;
            }
            out.push('\u{FFFD}');
        }
        if (0xD800..0xDC00).contains(&code) { pending_high = Some(code); continue; }
        out.push(char::from_u32(code).unwrap_or('\u{FFFD}'));
    }
    if pending_high.is_some() { out.push('\u{FFFD}'); }
    Ok(out)
}
