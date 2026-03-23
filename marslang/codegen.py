from __future__ import annotations

from dataclasses import dataclass, field
import copy
import pprint

from . import ast
from .errors import CodegenError


@dataclass
class InlineFunction:
    params: list[str]
    expr: ast.Node


@dataclass
class Scope:
    hot_values: dict[str, ast.Node] = field(default_factory=dict)
    inline_functions: dict[str, InlineFunction] = field(default_factory=dict)
    constants: set[str] = field(default_factory=set)

    def clone(self) -> "Scope":
        return Scope(
            hot_values=dict(self.hot_values),
            inline_functions=dict(self.inline_functions),
            constants=set(self.constants),
        )


class CodeGenerator:
    """Compile Marslang AST into a small Python-hosted VM program."""

    def __init__(self):
        self.scope = Scope()

    def generate(self, program: ast.Program) -> str:
        payload = self.serialize_program(program)
        encoded = pprint.pformat(payload, width=100, sort_dicts=True)
        return (
            "from marslang.runtime import execute_program\n\n"
            f"PROGRAM = {encoded}\n\n"
            "if __name__ == '__main__':\n"
            "    execute_program(PROGRAM)\n"
        )

    def serialize_program(self, program: ast.Program) -> dict:
        body = [self.serialize_stmt(node) for node in program.body if self.should_emit(node)]
        return {
            "kind": "Program",
            "body": body,
            "constants": sorted(self.scope.constants),
        }

    def should_emit(self, node: ast.Node) -> bool:
        if isinstance(node, ast.VarDecl) and node.is_hot:
            if not self.is_compile_time_constant(node.value):
                raise CodegenError(f"hot variable {node.name} must be compile-time constant")
            self.scope.hot_values[node.name] = node.value
            self.scope.constants.add(node.name)
            if node.is_fixed:
                self.scope.constants.add(node.name)
            return False
        if isinstance(node, ast.VarDecl) and node.is_fixed:
            self.scope.constants.add(node.name)
        if isinstance(node, ast.FunctionDecl):
            inline_expr = self.extract_inline_expr(node)
            if node.is_hot and inline_expr is not None:
                self.scope.inline_functions[node.name] = InlineFunction([param.name for param in node.params], inline_expr)
                self.scope.constants.add(node.name)
            elif node.is_fixed:
                self.scope.constants.add(node.name)
        return True

    def extract_inline_expr(self, node: ast.FunctionDecl) -> ast.Node | None:
        if node.expr_body is not None:
            return node.expr_body
        if node.body and len(node.body.statements) == 1 and isinstance(node.body.statements[0], ast.Return):
            return node.body.statements[0].value
        return None

    def is_compile_time_constant(self, node: ast.Node) -> bool:
        return isinstance(node, ast.Literal) or (
            isinstance(node, ast.ArrayLiteral) and all(self.is_compile_time_constant(item) for item in node.items)
        ) or (
            isinstance(node, ast.SetLiteral) and all(self.is_compile_time_constant(item) for item in node.items)
        ) or (
            isinstance(node, ast.PairLiteral)
            and self.is_compile_time_constant(node.first)
            and self.is_compile_time_constant(node.second)
        )

    def serialize_stmt(self, node: ast.Node) -> dict:
        method = getattr(self, f"serialize_{type(node).__name__}", None)
        if method is None:
            raise CodegenError(f"No serializer for {type(node).__name__}")
        return method(node)

    def serialize_type(self, type_ref: ast.TypeRef | None) -> dict | None:
        if type_ref is None:
            return None
        return {
            "kind": "TypeRef",
            "name": type_ref.name,
            "args": [self.serialize_type(arg) for arg in type_ref.args],
            "options": [self.serialize_type(opt) for opt in type_ref.options],
        }

    def serialize_ModuleImport(self, node: ast.ModuleImport) -> dict:
        return {"kind": "Import", "module": node.module, "alias": node.alias}

    def serialize_VarDecl(self, node: ast.VarDecl) -> dict:
        return {
            "kind": "VarDecl",
            "name": node.name,
            "type": self.serialize_type(node.type_ref),
            "value": self.serialize_expr(node.value, node.type_ref),
            "fixed": node.is_fixed,
        }

    def serialize_FunctionDecl(self, node: ast.FunctionDecl) -> dict:
        old_scope = self.scope.clone()
        for param in node.params:
            self.scope.hot_values.pop(param.name, None)
            self.scope.constants.discard(param.name)
        body = (
            [self.serialize_stmt(stmt) for stmt in node.body.statements]
            if node.body is not None
            else [{"kind": "Return", "value": self.serialize_expr(node.expr_body)}]
        )
        self.scope = old_scope
        return {
            "kind": "FunctionDecl",
            "name": node.name,
            "params": [
                {"name": param.name, "type": self.serialize_type(param.type_ref)}
                for param in node.params
            ],
            "body": body,
            "fixed": node.is_fixed,
            "hot": node.is_hot,
        }

    def serialize_FamilyDecl(self, node: ast.FamilyDecl) -> dict:
        body = []
        old_scope = self.scope.clone()
        for item in node.body:
            if isinstance(item, ast.FunctionDecl):
                body.append(self.serialize_FunctionDecl(item))
            elif isinstance(item, ast.VarDecl):
                body.append(self.serialize_VarDecl(item))
        self.scope = old_scope
        return {
            "kind": "FamilyDecl",
            "name": node.name,
            "base": node.base_name,
            "body": body,
        }

    def serialize_ExprStmt(self, node: ast.ExprStmt) -> dict:
        return {"kind": "ExprStmt", "expr": self.serialize_expr(node.expr)}

    def serialize_Return(self, node: ast.Return) -> dict:
        return {"kind": "Return", "value": None if node.value is None else self.serialize_expr(node.value)}

    def serialize_Assign(self, node: ast.Assign) -> dict:
        if isinstance(node.target, ast.Identifier) and node.target.name in self.scope.constants:
            raise CodegenError(f"Cannot reassign constant/hot variable {node.target.name}")
        return {
            "kind": "Assign",
            "target": self.serialize_target(node.target),
            "op": node.op,
            "value": self.serialize_expr(node.value),
        }

    def serialize_IfStmt(self, node: ast.IfStmt) -> dict:
        return {
            "kind": "IfStmt",
            "condition": self.serialize_expr(node.condition),
            "then": [self.serialize_stmt(stmt) for stmt in node.then_block.statements],
            "elifs": [
                {
                    "condition": self.serialize_expr(cond),
                    "body": [self.serialize_stmt(stmt) for stmt in block.statements],
                }
                for cond, block in node.elif_blocks
            ],
            "else": [] if node.else_block is None else [self.serialize_stmt(stmt) for stmt in node.else_block.statements],
        }

    def serialize_RepeatStmt(self, node: ast.RepeatStmt) -> dict:
        return {
            "kind": "RepeatStmt",
            "count": self.serialize_expr(node.count),
            "body": [self.serialize_stmt(stmt) for stmt in node.body.statements],
        }

    def serialize_ForStmt(self, node: ast.ForStmt) -> dict:
        return {
            "kind": "ForStmt",
            "init": [self.serialize_for_component(item) for item in node.init],
            "condition": self.serialize_expr(node.condition),
            "update": [self.serialize_for_component(item) for item in node.update],
            "body": [self.serialize_stmt(stmt) for stmt in node.body.statements],
        }

    def serialize_for_component(self, node: ast.Node) -> dict:
        if isinstance(node, ast.Assign):
            return self.serialize_Assign(node)
        return {"kind": "ExprStmt", "expr": self.serialize_expr(node)}

    def serialize_MatchStmt(self, node: ast.MatchStmt) -> dict:
        return {
            "kind": "MatchStmt",
            "subject": self.serialize_expr(node.subject),
            "cases": [
                {
                    "default": case.is_default,
                    "pattern": None if case.pattern is None else self.serialize_expr(case.pattern),
                    "body": [self.serialize_stmt(stmt) for stmt in case.block.statements],
                }
                for case in node.cases
            ],
        }

    def serialize_TryStmt(self, node: ast.TryStmt) -> dict:
        return {
            "kind": "TryStmt",
            "body": [self.serialize_stmt(stmt) for stmt in node.body.statements],
            "handlers": node.handlers,
            "handler_body": [self.serialize_stmt(stmt) for stmt in node.handler_block.statements],
            "then_body": [] if node.finally_block is None else [self.serialize_stmt(stmt) for stmt in node.finally_block.statements],
        }

    def serialize_target(self, node: ast.Node) -> dict:
        if isinstance(node, ast.Identifier):
            return {"kind": "Identifier", "name": node.name}
        if isinstance(node, ast.Attr):
            return {
                "kind": "Attr",
                "obj": self.serialize_expr(node.obj),
                "name": node.name,
            }
        raise CodegenError(f"Unsupported assignment target {type(node).__name__}")

    def serialize_expr(self, node: ast.Node, type_ref: ast.TypeRef | None = None) -> dict:
        if isinstance(node, ast.Identifier):
            if node.name in self.scope.hot_values:
                return self.serialize_expr(self.scope.hot_values[node.name])
            return {"kind": "Identifier", "name": node.name}
        if isinstance(node, ast.Literal):
            return {"kind": "Literal", "value": node.value, "literal_kind": node.literal_kind}
        if isinstance(node, ast.ArrayLiteral):
            return {
                "kind": "ArrayLiteral",
                "items": [self.serialize_expr(item) for item in node.items],
                "type": self.serialize_type(type_ref),
            }
        if isinstance(node, ast.SetLiteral):
            return {
                "kind": "SetLiteral",
                "items": [self.serialize_expr(item) for item in node.items],
                "type": self.serialize_type(type_ref),
            }
        if isinstance(node, ast.PairLiteral):
            return {
                "kind": "PairLiteral",
                "first": self.serialize_expr(node.first),
                "second": self.serialize_expr(node.second),
                "type": self.serialize_type(type_ref),
            }
        if isinstance(node, ast.UnaryOp):
            return {"kind": "UnaryOp", "op": node.op, "operand": self.serialize_expr(node.operand)}
        if isinstance(node, ast.BinaryOp):
            return {
                "kind": "BinaryOp",
                "left": self.serialize_expr(node.left),
                "op": node.op,
                "right": self.serialize_expr(node.right),
            }
        if isinstance(node, ast.Call):
            if isinstance(node.func, ast.Identifier) and node.func.name in self.scope.inline_functions:
                inline = self.scope.inline_functions[node.func.name]
                if len(inline.params) != len(node.args):
                    raise CodegenError(f"Inline function {node.func.name} called with wrong argument count")
                mapping = dict(zip(inline.params, node.args))
                return self.serialize_expr(self.substitute(copy.deepcopy(inline.expr), mapping))
            return {
                "kind": "Call",
                "func": self.serialize_expr(node.func),
                "args": [self.serialize_expr(arg) for arg in node.args],
            }
        if isinstance(node, ast.Attr):
            return {"kind": "Attr", "obj": self.serialize_expr(node.obj), "name": node.name}
        if isinstance(node, ast.RangeExpr):
            return {"kind": "RangeExpr", "start": self.serialize_expr(node.start), "end": self.serialize_expr(node.end)}
        if isinstance(node, ast.Assign):
            return self.serialize_Assign(node)
        raise CodegenError(f"Unsupported expression node {type(node).__name__}")

    def substitute(self, node: ast.Node, mapping: dict[str, ast.Node]) -> ast.Node:
        if isinstance(node, ast.Identifier) and node.name in mapping:
            return mapping[node.name]
        for field_name, value in vars(node).items():
            if field_name in {"line", "column"}:
                continue
            if isinstance(value, ast.Node):
                setattr(node, field_name, self.substitute(value, mapping))
            elif isinstance(value, list):
                setattr(node, field_name, [self.substitute(item, mapping) if isinstance(item, ast.Node) else item for item in value])
        return node
