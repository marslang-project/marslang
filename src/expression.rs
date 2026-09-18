use crate::ast::Expr;

#[derive(Clone, Debug)]
enum Token { Name(String), Number(String), String(String), Op(String), End }

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
            tokens.push(Token::String(text[start..end.ok_or("unterminated string")?].into()));
        } else {
            let mut op = ch.to_string();
            if let Some((_, next)) = chars.peek() {
                let combined = format!("{ch}{next}");
                if matches!(combined.as_str(), "==" | "!=" | "<=" | ">=" | "**") {
                    op = combined; chars.next();
                }
            }
            if !matches!(op.as_str(), "+" | "-" | "*" | "/" | "%" | "**" | "=" | "==" | "!=" | "<" | ">" | "<=" | ">=" | "(" | ")" | "," | ".") {
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
