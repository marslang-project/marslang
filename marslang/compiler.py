from __future__ import annotations

from dataclasses import dataclass
from pathlib import Path

from . import ast
from .codegen import CodeGenerator
from .lexer import Lexer
from .parser import Parser
from .tokens import Token


@dataclass(slots=True)
class CompilationResult:
    input_path: Path | None
    output_path: Path | None
    source: str
    tokens: list[Token]
    program: ast.Program
    python_source: str

    @property
    def token_count(self) -> int:
        return max(len(self.tokens) - 1, 0)

    @property
    def top_level_count(self) -> int:
        return len(self.program.body)


def analyze_source(source: str) -> tuple[list[Token], ast.Program]:
    tokens = Lexer(source).tokenize()
    program = Parser(tokens).parse()
    return tokens, program


def compile_source(source: str) -> str:
    tokens, program = analyze_source(source)
    return CodeGenerator().generate(program)


def compile_source_detailed(source: str, input_path: str | Path | None = None) -> CompilationResult:
    tokens, program = analyze_source(source)
    python_source = CodeGenerator().generate(program)
    return CompilationResult(
        input_path=None if input_path is None else Path(input_path),
        output_path=None,
        source=source,
        tokens=tokens,
        program=program,
        python_source=python_source,
    )


def compile_file(input_path: str | Path, output_path: str | Path | None = None) -> Path:
    result = compile_file_detailed(input_path, output_path)
    return result.output_path


def compile_file_detailed(input_path: str | Path, output_path: str | Path | None = None) -> CompilationResult:
    input_path = Path(input_path)
    source = input_path.read_text(encoding="utf-8")
    result = compile_source_detailed(source, input_path=input_path)
    if output_path is None:
        output_path = input_path.with_suffix(".py")
    output_path = Path(output_path)
    output_path.write_text(result.python_source, encoding="utf-8")
    result.output_path = output_path
    return result
