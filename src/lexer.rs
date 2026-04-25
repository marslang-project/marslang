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
            i += 1;
            while i < chars.len() && chars[i] != q {
                i += 1;
            }
            if i < chars.len() {
                i += 1;
            }
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
