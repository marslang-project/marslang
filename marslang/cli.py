from __future__ import annotations

import argparse
from pathlib import Path
import runpy
import sys

from .compiler import CompilationResult, compile_file_detailed
from .errors import MarslangError


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(
        prog="compiler",
        description="Compile Marslang .mrs files into Python VM runner files.",
    )
    parser.add_argument("input", help="Input .mrs source file")
    parser.add_argument("-o", "--output", help="Output file path")
    parser.add_argument("--run", action="store_true", help="Run compiled output after emitting it")
    parser.add_argument("-v", "--verbose", action="store_true", help="Print detailed compilation progress")
    return parser


def verbose_print(enabled: bool, message: str) -> None:
    if enabled:
        print(f"[marslang] {message}")


def print_summary(result: CompilationResult) -> None:
    print(
        "[marslang] summary: "
        f"tokens={result.token_count}, top_level_nodes={result.top_level_count}, output={result.output_path}"
    )


def main(argv: list[str] | None = None) -> int:
    args = build_parser().parse_args(argv)
    input_path = Path(args.input)
    if not input_path.exists():
        print(f"marslang compile error: input file not found: {input_path}", file=sys.stderr)
        return 1
    try:
        verbose_print(args.verbose, f"reading source from {input_path}")
        result = compile_file_detailed(input_path, args.output)
    except (MarslangError, OSError) as exc:
        print(f"marslang compile error: {exc}", file=sys.stderr)
        return 1
    verbose_print(args.verbose, f"lexed {result.token_count} tokens")
    verbose_print(args.verbose, f"parsed {result.top_level_count} top-level nodes")
    verbose_print(args.verbose, f"wrote VM runner to {result.output_path}")
    print(f"Compiled {input_path} -> {result.output_path}")
    if args.verbose:
        print_summary(result)
    if args.run:
        verbose_print(args.verbose, f"running {result.output_path}")
        try:
            runpy.run_path(str(Path(result.output_path).resolve()), run_name="__main__")
        except Exception as exc:  # runtime execution should be user-friendly at the CLI boundary
            print(f"marslang runtime error: {exc}", file=sys.stderr)
            return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
