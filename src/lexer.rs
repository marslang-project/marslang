#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TokenKind {
    Keyword(String),
    Ident(String),
    Number(String),
    String(String),
    Symbol(char),
}

#[derive(Debug, Clone)]
pub struct Token {
    pub kind: TokenKind,
    pub pos: usize,
}

pub fn lex(input: &str) -> Vec<Token> {
    let mut out = Vec::new();
    let chars: Vec<char> = input.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        if c.is_whitespace() {
            i += 1;
            continue;
        }
        if c.is_ascii_alphabetic() || c == '_' {
            let start = i;
            i += 1;
            while i < chars.len() && (chars[i].is_ascii_alphanumeric() || chars[i] == '_') {
                i += 1;
            }
            let s: String = chars[start..i].iter().collect();
            let kind = if matches!(
                s.as_str(),
                "func" | "family" | "hot" | "cold" | "fixed" | "ret" | "takepkg"
            ) {
                TokenKind::Keyword(s)
            } else {
                TokenKind::Ident(s)
            };
            out.push(Token { kind, pos: start });
            continue;
        }
        if c.is_ascii_digit() {
            let start = i;
            i += 1;
            while i < chars.len() && (chars[i].is_ascii_digit() || chars[i] == '.') {
                i += 1;
            }
            out.push(Token {
                kind: TokenKind::Number(chars[start..i].iter().collect()),
                pos: start,
            });
            continue;
        }
        if c == '"' || c == '\'' {
            let q = c;
            let start = i;
            // """...""" spans lines and ends at the next three quotes.
            let triple = q == '"' && chars.get(i + 1) == Some(&'"') && chars.get(i + 2) == Some(&'"');
            i += if triple { 3 } else { 1 };
            while i < chars.len() {
                if chars[i] == '\\' {
                    i += 2;
                    continue;
                }
                let closes = if triple {
                    chars[i] == '"' && chars.get(i + 1) == Some(&'"') && chars.get(i + 2) == Some(&'"')
                } else {
                    chars[i] == q
                };
                if closes {
                    i += if triple { 3 } else { 1 };
                    break;
                }
                i += 1;
            }
            i = i.min(chars.len());
            out.push(Token {
                kind: TokenKind::String(chars[start..i].iter().collect()),
                pos: start,
            });
            continue;
        }

        out.push(Token {
            kind: TokenKind::Symbol(c),
            pos: i,
        });
        i += 1;
    }
    out
}
