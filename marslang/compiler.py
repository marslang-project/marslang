from __future__ import annotations

from pathlib import Path

from .codegen import CodeGenerator
from .lexer import Lexer
from .parser import Parser


def compile_source(source: str) -> str:
    tokens = Lexer(source).tokenize()
    program = Parser(tokens).parse()
    return CodeGenerator().generate(program)


def compile_file(input_path: str | Path, output_path: str | Path | None = None) -> Path:
    input_path = Path(input_path)
    source = input_path.read_text()
    compiled = compile_source(source)
    if output_path is None:
        output_path = input_path.with_suffix(".py")
    output_path = Path(output_path)
    output_path.write_text(compiled)
    return output_path
