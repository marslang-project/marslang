from __future__ import annotations

from dataclasses import dataclass


@dataclass(frozen=True)
class Token:
    kind: str
    value: str
    line: int
    column: int

    def display(self) -> str:
        return f"{self.kind}({self.value!r}) at {self.line}:{self.column}"
