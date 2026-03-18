from __future__ import annotations

from dataclasses import dataclass, field
from typing import Optional
import copy
import json

from . import ast
from .errors import CodegenError
from .runtime import RUNTIME_HEADER


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
    def __init__(self):
        self.lines: list[str] = []
        self.indent = 0
        self.scope = Scope()

    def emit(self, line: str = "") -> None:
        self.lines.append("    " * self.indent + line)

    def generate(self, program: ast.Program) -> str:
        self.emit(RUNTIME_HEADER.rstrip())
        self.emit()
        for node in program.body:
            self.gen_node(node)
            if self.lines and self.lines[-1] != "":
                self.emit()
        return "\n".join(self.lines).rstrip() + "\n"

    def gen_node(self, node: ast.Node) -> None:
        method = getattr(self, f"gen_{type(node).__name__}", None)
        if method is None:
            raise CodegenError(f"No codegen for {type(node).__name__}")
        method(node)

    def gen_ModuleImport(self, node: ast.ModuleImport) -> None:
        if node.alias:
            self.emit(f"import {node.module} as {node.alias}")
        else:
            self.emit(f"import {node.module}")

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

    def gen_VarDecl(self, node: ast.VarDecl) -> None:
        if node.is_hot:
            if not self.is_compile_time_constant(node.value):
                raise CodegenError(f"hot variable {node.name} must be compile-time constant")
            self.scope.hot_values[node.name] = node.value
        if node.is_fixed or node.is_hot:
            self.scope.constants.add(node.name)
        if not node.is_hot:
            self.emit(f"{node.name} = {self.expr(node.value, node.type_ref)}")

    def gen_FunctionDecl(self, node: ast.FunctionDecl) -> None:
        inline_expr = node.expr_body
        if node.is_hot and inline_expr is None and node.body is not None and len(node.body.statements) == 1 and isinstance(node.body.statements[0], ast.Return):
            inline_expr = node.body.statements[0].value
        if node.is_hot and inline_expr is not None:
            self.scope.inline_functions[node.name] = InlineFunction([param.name for param in node.params], inline_expr)
        params = ", ".join(param.name for param in node.params)
        py_name = "__mars_main__" if node.name == "m" else node.name
        self.emit(f"def {py_name}({params}):")
        self.indent += 1
        old_scope = self.scope.clone()
        for param in node.params:
            self.scope.hot_values.pop(param.name, None)
            self.scope.constants.discard(param.name)
        if node.expr_body is not None:
            self.emit(f"return {self.expr(node.expr_body)}")
        elif node.body and node.body.statements:
            for stmt in node.body.statements:
                self.gen_node(stmt)
        else:
            self.emit("pass")
        self.scope = old_scope
        self.indent -= 1
        if node.name == "m":
            self.emit()
            self.emit("if __name__ == '__main__':")
            self.indent += 1
            self.emit("__mars_main__()")
            self.indent -= 1

    def gen_FamilyDecl(self, node: ast.FamilyDecl) -> None:
        base = node.base_name or "object"
        self.emit(f"class {node.name}({base}):")
        self.indent += 1
        if not node.body:
            self.emit("pass")
        else:
            had_body = False
            for item in node.body:
                if isinstance(item, ast.FunctionDecl):
                    had_body = True
                    params = ["self"] + [p.name for p in item.params]
                    py_name = "__init__" if item.name == "init" else item.name
                    self.emit(f"def {py_name}({', '.join(params)}):")
                    self.indent += 1
                    if item.expr_body is not None:
                        self.emit(f"return {self.expr(item.expr_body)}")
                    elif item.body and item.body.statements:
                        old_scope = self.scope.clone()
                        for stmt in item.body.statements:
                            self.gen_node(stmt)
                        self.scope = old_scope
                    else:
                        self.emit("pass")
                    self.indent -= 1
                elif isinstance(item, ast.VarDecl):
                    had_body = True
                    self.emit(f"{item.name} = {self.expr(item.value, item.type_ref)}")
            if not had_body:
                self.emit("pass")
        self.indent -= 1

    def gen_ExprStmt(self, node: ast.ExprStmt) -> None:
        self.emit(self.expr(node.expr))

    def gen_Return(self, node: ast.Return) -> None:
        if node.value is None:
            self.emit("return")
        else:
            self.emit(f"return {self.expr(node.value)}")

    def gen_Assign(self, node: ast.Assign) -> None:
        if isinstance(node.target, ast.Identifier) and node.target.name in self.scope.constants:
            raise CodegenError(f"Cannot reassign constant/hot variable {node.target.name}")
        target = self.expr(node.target)
        if node.op == "=":
            self.emit(f"{target} = {self.expr(node.value)}")
        elif node.op == "+=":
            self.emit(f"{target} += {self.expr(node.value)}")
        elif node.op == "-=":
            self.emit(f"{target} -= {self.expr(node.value)}")
        else:
            raise CodegenError(f"Unsupported assignment operator {node.op}")

    def gen_IfStmt(self, node: ast.IfStmt) -> None:
        self.emit(f"if {self.expr(node.condition)}:")
        self.block(node.then_block)
        for cond, block in node.elif_blocks:
            self.emit(f"elif {self.expr(cond)}:")
            self.block(block)
        if node.else_block is not None:
            self.emit("else:")
            self.block(node.else_block)

    def gen_RepeatStmt(self, node: ast.RepeatStmt) -> None:
        self.emit(f"for __mars_repeat_index in range({self.expr(node.count)}):")
        self.block(node.body)

    def gen_ForStmt(self, node: ast.ForStmt) -> None:
        for init in node.init:
            self.emit(self.statement_expr(init))
        self.emit(f"while {self.expr(node.condition)}:")
        self.indent += 1
        if node.body and node.body.statements:
            for stmt in node.body.statements:
                self.gen_node(stmt)
        for update in node.update:
            self.emit(self.statement_expr(update))
        if not node.body or not node.body.statements:
            self.emit("pass")
        self.indent -= 1

    def gen_MatchStmt(self, node: ast.MatchStmt) -> None:
        subject = self.expr(node.subject)
        temp = "__mars_match_subject"
        self.emit(f"{temp} = {subject}")
        first = True
        for case in node.cases:
            if case.is_default:
                self.emit("else:")
            else:
                condition = self.match_condition(temp, case.pattern)
                prefix = "if" if first else "elif"
                self.emit(f"{prefix} {condition}:")
                first = False
            self.block(case.block)

    def gen_TryStmt(self, node: ast.TryStmt) -> None:
        self.emit("try:")
        self.block(node.body)
        errors = ", ".join(node.handlers) or "Error"
        self.emit(f"except ({errors}) as __mars_error:")
        self.block(node.handler_block)
        if node.finally_block is not None:
            self.emit("finally:")
            self.block(node.finally_block)

    def block(self, block: Optional[ast.Block]) -> None:
        self.indent += 1
        if block is None or not block.statements:
            self.emit("pass")
        else:
            for stmt in block.statements:
                self.gen_node(stmt)
        self.indent -= 1

    def statement_expr(self, node: ast.Node) -> str:
        if isinstance(node, ast.Assign):
            target = self.expr(node.target)
            return f"{target} {node.op} {self.expr(node.value)}"
        return self.expr(node)

    def match_condition(self, subject_name: str, pattern: ast.Node) -> str:
        if isinstance(pattern, ast.RangeExpr):
            return f"({self.expr(pattern.start)} <= {subject_name} < {self.expr(pattern.end)})"
        return f"{subject_name} == {self.expr(pattern)}"

    def expr(self, node: ast.Node, type_ref: ast.TypeRef | None = None) -> str:
        if isinstance(node, ast.Identifier):
            if node.name == "me":
                return "self"
            if node.name == "in":
                return "in_"
            if node.name in self.scope.hot_values:
                return self.expr(self.scope.hot_values[node.name])
            return node.name
        if isinstance(node, ast.Literal):
            return json.dumps(node.value)
        if isinstance(node, ast.ArrayLiteral):
            inner = ", ".join(self.expr(item) for item in node.items)
            type_name = self.extract_container_type(type_ref)
            if type_name:
                return f"MArray([{inner}], type_name={type_name!r})"
            return f"a({inner})"
        if isinstance(node, ast.SetLiteral):
            inner = ", ".join(self.expr(item) for item in node.items)
            type_name = self.extract_container_type(type_ref)
            if type_name:
                return f"MSet([{inner}], type_name={type_name!r})"
            return f"s({inner})"
        if isinstance(node, ast.PairLiteral):
            return f"p({self.expr(node.first)}, {self.expr(node.second)})"
        if isinstance(node, ast.UnaryOp):
            op = "not" if node.op == "not" else node.op
            return f"({op} {self.expr(node.operand)})"
        if isinstance(node, ast.BinaryOp):
            op_map = {"and": "and", "or": "or", "and/or": "and"}
            op = op_map.get(node.op, node.op)
            return f"({self.expr(node.left)} {op} {self.expr(node.right)})"
        if isinstance(node, ast.Call):
            if isinstance(node.func, ast.Identifier) and node.func.name in self.scope.inline_functions:
                inline = self.scope.inline_functions[node.func.name]
                if len(inline.params) != len(node.args):
                    raise CodegenError(f"Inline function {node.func.name} called with wrong argument count")
                mapping = dict(zip(inline.params, node.args))
                return self.expr(self.substitute(copy.deepcopy(inline.expr), mapping))
            func_expr = self.expr(node.func)
            args = ", ".join(self.expr(arg) for arg in node.args)
            return f"{func_expr}({args})"
        if isinstance(node, ast.Attr):
            return f"{self.expr(node.obj)}.{node.name}"
        if isinstance(node, ast.Assign):
            target = self.expr(node.target)
            return f"({target} {node.op} {self.expr(node.value)})"
        if isinstance(node, ast.RangeExpr):
            return f"range({self.expr(node.start)}, {self.expr(node.end)})"
        raise CodegenError(f"Unsupported expression node {type(node).__name__}")

    def extract_container_type(self, type_ref: ast.TypeRef | None) -> str | None:
        if type_ref is None:
            return None
        if type_ref.name in {"array", "set"} and type_ref.args:
            return type_ref.args[0].name
        return None

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
