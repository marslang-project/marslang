from __future__ import annotations

from .errors import LexerError
from .tokens import Token

KEYWORDS = {
    "takepkg",
    "fixed",
    "hot",
    "cold",
    "func",
    "family",
    "ret",
    "if",
    "elif",
    "else",
    "repeat",
    "match",
    "for",
    "run",
    "handle",
    "then",
    "err",
    "true",
    "false",
    "fasle",
    "null",
    "also",
    "or",
    "and",
    "and/or",
    "not",
}

MULTI_OPS = [
    "=>",
    "==",
    "!=",
    "<=",
    ">=",
    "+=",
    "-=",
    "++",
    "--",
    "**",
    "__",
]

SINGLE_OPS = set("+-*/%=<>(){}[];,.:")


class Lexer:
    def __init__(self, source: str):
        self.source = source
        self.length = len(source)
        self.pos = 0
        self.line = 1
        self.column = 1

    def peek(self, offset: int = 0) -> str:
        idx = self.pos + offset
        if idx >= self.length:
            return ""
        return self.source[idx]

    def advance(self, count: int = 1) -> str:
        result = ""
        for _ in range(count):
            if self.pos >= self.length:
                break
            ch = self.source[self.pos]
            result += ch
            self.pos += 1
            if ch == "\n":
                self.line += 1
                self.column = 1
            else:
                self.column += 1
        return result

    def token(self, kind: str, value: str, line: int, column: int) -> Token:
        return Token(kind, value, line, column)

    def skip_ws_and_comments(self) -> None:
        while True:
            ch = self.peek()
            if ch and ch.isspace():
                self.advance()
                continue
            if ch == "/" and self.peek(1) == "/":
                self.advance(2)
                while self.peek() not in {"", "\n"}:
                    self.advance()
                continue
            if ch == "/" and self.peek(1) == "*":
                self.advance(2)
                while not (self.peek() == "*" and self.peek(1) == "/"):
                    if self.peek() == "":
                        raise LexerError("Unterminated block comment")
                    self.advance()
                self.advance(2)
                continue
            break

    def lex_identifier(self) -> Token:
        line, col = self.line, self.column
        value = ""
        while True:
            ch = self.peek()
            if ch.isalnum() or ch == "_":
                value += self.advance()
                continue
            if ch == "/" and self.source[self.pos:self.pos + 6] == "and/or":
                value += self.advance(6)
                continue
            break
        kind = "KEYWORD" if value in KEYWORDS else "IDENT"
        return self.token(kind, value, line, col)

    def lex_number(self) -> Token:
        line, col = self.line, self.column
        value = ""
        has_dot = False
        while True:
            ch = self.peek()
            if ch.isdigit():
                value += self.advance()
            elif ch == "." and not has_dot and self.peek(1).isdigit():
                has_dot = True
                value += self.advance()
            else:
                break
        return self.token("NUMBER", value, line, col)

    def lex_string(self) -> Token:
        line, col = self.line, self.column
        quote = self.peek()
        if self.source[self.pos:self.pos + 3] == quote * 3:
            self.advance(3)
            value = ""
            while self.source[self.pos:self.pos + 3] != quote * 3:
                if self.peek() == "":
                    raise LexerError("Unterminated multiline string")
                if self.peek() == "\\":
                    value += self.advance()
                    if self.peek() == "":
                        break
                value += self.advance()
            self.advance(3)
            return self.token("STRING", value, line, col)
        self.advance()
        value = ""
        while self.peek() != quote:
            if self.peek() == "":
                raise LexerError("Unterminated string")
            if self.peek() == "\\":
                value += self.advance()
                if self.peek() == "":
                    break
            value += self.advance()
        self.advance()
        return self.token("STRING", value, line, col)

    def tokenize(self) -> list[Token]:
        tokens: list[Token] = []
        while True:
            self.skip_ws_and_comments()
            ch = self.peek()
            if ch == "":
                break
            if ch.isalpha() or ch == "_":
                tokens.append(self.lex_identifier())
                continue
            if ch.isdigit():
                tokens.append(self.lex_number())
                continue
            if ch in {'"', "'"}:
                tokens.append(self.lex_string())
                continue
            line, col = self.line, self.column
            matched = False
            for op in MULTI_OPS:
                if self.source.startswith(op, self.pos):
                    self.advance(len(op))
                    tokens.append(self.token(op, op, line, col))
                    matched = True
                    break
            if matched:
                continue
            if ch in SINGLE_OPS:
                self.advance()
                tokens.append(self.token(ch, ch, line, col))
                continue
            raise LexerError(f"Unexpected character {ch!r} at {line}:{col}")
        tokens.append(self.token("EOF", "", self.line, self.column))
        return tokens
