from __future__ import annotations

import argparse
from pathlib import Path
import runpy
import sys

from .compiler import compile_file
from .errors import MarslangError


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(prog="compiler", description="Compile Marslang .mrs files into Python.")
    parser.add_argument("input", help="Input .mrs source file")
    parser.add_argument("-o", "--output", help="Output file path")
    parser.add_argument("--run", action="store_true", help="Run compiled output after emitting it")
    return parser


def main(argv: list[str] | None = None) -> int:
    args = build_parser().parse_args(argv)
    try:
        output = compile_file(args.input, args.output)
    except MarslangError as exc:
        print(f"marslang compile error: {exc}", file=sys.stderr)
        return 1
    print(f"Compiled {args.input} -> {output}")
    if args.run:
        runpy.run_path(str(Path(output).resolve()), run_name="__main__")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
