use crate::ast::Expr;

#[derive(Clone, Debug)]
enum Token { Name(String), Number(String), String(String), Op(String), Lambda(String), End }

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
                    tokens.push(Token::Lambda(text[open + 1..close].to_string()));
                    continue;
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
    let mut parser = Parser { tokens, pos: 0 };
    let value = parser.expr(0)?;
    if !matches!(parser.tokens[parser.pos], Token::End) {
        return Err(format!("unexpected expression token {:?}", parser.tokens[parser.pos]));
    }
    Ok(value)
}

struct Parser { tokens: Vec<Token>, pos: usize }
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
        let mut left = match self.take() {
            Token::Name(s) => match s.as_str() {
                "true" => Expr::Bool(true), "false" | "fasle" => Expr::Bool(false),
                "null" => Expr::Null, _ => Expr::Ident(s),
            },
            Token::Number(s) => Expr::Number(s), Token::String(s) => Expr::String(s),
            Token::Lambda(params) => {
                let body = self.expr(0)?;
                Expr::Lambda(std::sync::Arc::new(crate::parser::lambda(&params, body)?))
            }
            Token::Op(s) if s == "(" => { let v = self.expr(0)?; self.expect(")")?; v }
            Token::Op(op) if matches!(op.as_str(), "+" | "-" | "not") => {
                let value = self.expr(if op == "not" { 3 } else { 6 })?;
                Expr::Unary { op, value: Box::new(value) }
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
            let op = op.clone(); self.pos += 1;
            let right = self.expr(precedence + u8::from(!right_assoc))?;
            left = Expr::Binary { left: Box::new(left), op, right: Box::new(right) };
        }
        Ok(left)
    }
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
