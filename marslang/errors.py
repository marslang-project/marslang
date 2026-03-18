class MarslangError(Exception):
    """Base Marslang compiler error."""


class LexerError(MarslangError):
    pass


class ParserError(MarslangError):
    pass


class CodegenError(MarslangError):
    pass
