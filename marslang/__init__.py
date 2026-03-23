"""Marslang v0.4 compiler package."""

from .compiler import (
    CompilationResult,
    analyze_source,
    compile_file,
    compile_file_detailed,
    compile_source,
    compile_source_detailed,
)

__all__ = [
    "CompilationResult",
    "analyze_source",
    "compile_source",
    "compile_source_detailed",
    "compile_file",
    "compile_file_detailed",
]
