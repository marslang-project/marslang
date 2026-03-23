from __future__ import annotations

from dataclasses import dataclass, field
from typing import Optional


@dataclass
class Node:
    line: int = 0
    column: int = 0


@dataclass
class Program(Node):
    body: list[Node] = field(default_factory=list)


@dataclass
class TypeRef(Node):
    name: str = ""
    args: list[TypeRef] = field(default_factory=list)
    options: list[TypeRef] = field(default_factory=list)


@dataclass
class Param(Node):
    name: str = ""
    type_ref: Optional[TypeRef] = None


@dataclass
class Block(Node):
    statements: list[Node] = field(default_factory=list)


@dataclass
class ModuleImport(Node):
    module: str = ""
    alias: Optional[str] = None


@dataclass
class VarDecl(Node):
    name: str = ""
    type_ref: Optional[TypeRef] = None
    value: Node | None = None
    is_hot: bool = False
    is_fixed: bool = False
    is_cold: bool = False


@dataclass
class Assign(Node):
    target: Node | None = None
    value: Node | None = None
    op: str = "="


@dataclass
class ExprStmt(Node):
    expr: Node | None = None


@dataclass
class Return(Node):
    value: Node | None = None


@dataclass
class FunctionDecl(Node):
    name: str = ""
    params: list[Param] = field(default_factory=list)
    body: Block | None = None
    expr_body: Node | None = None
    is_hot: bool = False
    is_fixed: bool = False
    is_cold: bool = False


@dataclass
class FamilyDecl(Node):
    name: str = ""
    base_name: Optional[str] = None
    body: list[Node] = field(default_factory=list)


@dataclass
class IfStmt(Node):
    condition: Node | None = None
    then_block: Block | None = None
    elif_blocks: list[tuple[Node, Block]] = field(default_factory=list)
    else_block: Optional[Block] = None


@dataclass
class RepeatStmt(Node):
    count: Node | None = None
    body: Block | None = None


@dataclass
class ForStmt(Node):
    init: list[Node] = field(default_factory=list)
    condition: Node | None = None
    update: list[Node] = field(default_factory=list)
    body: Block | None = None


@dataclass
class MatchCase(Node):
    pattern: Node | None = None
    block: Block | None = None
    is_default: bool = False


@dataclass
class MatchStmt(Node):
    subject: Node | None = None
    cases: list[MatchCase] = field(default_factory=list)


@dataclass
class TryStmt(Node):
    body: Block | None = None
    handlers: list[str] = field(default_factory=list)
    handler_block: Block | None = None
    finally_block: Optional[Block] = None


@dataclass
class Identifier(Node):
    name: str = ""


@dataclass
class Literal(Node):
    value: object = None
    literal_kind: str = "literal"


@dataclass
class ArrayLiteral(Node):
    items: list[Node] = field(default_factory=list)


@dataclass
class SetLiteral(Node):
    items: list[Node] = field(default_factory=list)


@dataclass
class PairLiteral(Node):
    first: Node | None = None
    second: Node | None = None


@dataclass
class UnaryOp(Node):
    op: str = ""
    operand: Node | None = None


@dataclass
class BinaryOp(Node):
    left: Node | None = None
    op: str = ""
    right: Node | None = None


@dataclass
class Call(Node):
    func: Node | None = None
    args: list[Node] = field(default_factory=list)


@dataclass
class Attr(Node):
    obj: Node | None = None
    name: str = ""


@dataclass
class Index(Node):
    obj: Node | None = None
    index: Node | None = None


@dataclass
class RangeExpr(Node):
    start: Node | None = None
    end: Node | None = None
