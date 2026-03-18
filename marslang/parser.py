from __future__ import annotations

from . import ast
from .errors import ParserError
from .tokens import Token

PRECEDENCE = {
    "or": 1,
    "and": 2,
    "and/or": 2,
    "==": 3,
    "!=": 3,
    "<": 4,
    "<=": 4,
    ">": 4,
    ">=": 4,
    "+": 5,
    "-": 5,
    "*": 6,
    "/": 6,
    "%": 6,
    "**": 7,
}

ASSIGN_OPS = {"=", "+=", "-="}


class Parser:
    def __init__(self, tokens: list[Token]):
        self.tokens = tokens
        self.pos = 0

    def current(self) -> Token:
        return self.tokens[self.pos]

    def previous(self) -> Token:
        return self.tokens[self.pos - 1]

    def at(self, *kinds: str) -> bool:
        tok = self.current()
        return tok.kind in kinds or tok.value in kinds

    def match(self, *kinds: str) -> bool:
        if self.at(*kinds):
            self.pos += 1
            return True
        return False

    def expect(self, *kinds: str) -> Token:
        if not self.at(*kinds):
            expected = ", ".join(kinds)
            raise ParserError(f"Expected {expected}, got {self.current().display()}")
        tok = self.current()
        self.pos += 1
        return tok

    def parse(self) -> ast.Program:
        body: list[ast.Node] = []
        while not self.at("EOF"):
            body.append(self.parse_toplevel())
        return ast.Program(body=body)

    def parse_toplevel(self) -> ast.Node:
        if self.at("KEYWORD") and self.current().value == "takepkg":
            return self.parse_import()
        if self.peek_decl_start():
            return self.parse_decl_or_stmt()
        if self.at("KEYWORD") and self.current().value == "func":
            return self.parse_function()
        if self.at("KEYWORD") and self.current().value == "family":
            return self.parse_family()
        return self.parse_statement()

    def peek_decl_start(self) -> bool:
        idx = self.pos
        while idx < len(self.tokens) and self.tokens[idx].kind == "KEYWORD" and self.tokens[idx].value in {"fixed", "hot", "cold"}:
            idx += 1
        if idx < len(self.tokens) and self.tokens[idx].kind == "KEYWORD" and self.tokens[idx].value == "func":
            return True
        return self.looks_like_variable_decl(idx)

    def looks_like_variable_decl(self, idx: int) -> bool:
        if idx >= len(self.tokens) or self.tokens[idx].kind != "IDENT":
            return False
        idx += 1
        if idx < len(self.tokens) and self.tokens[idx].value == "(":
            depth = 1
            idx += 1
            while idx < len(self.tokens) and depth > 0:
                if self.tokens[idx].value == "(":
                    depth += 1
                elif self.tokens[idx].value == ")":
                    depth -= 1
                idx += 1
            if depth != 0:
                return False
        return idx < len(self.tokens) and self.tokens[idx].value in {"=", "+=", "-="}

    def parse_modifiers(self) -> tuple[bool, bool]:
        is_fixed = False
        is_hot = False
        seen_cold = False
        while self.at("KEYWORD") and self.current().value in {"fixed", "hot", "cold"}:
            value = self.current().value
            self.pos += 1
            if value == "fixed":
                is_fixed = True
            elif value == "hot":
                is_hot = True
            elif value == "cold":
                seen_cold = True
        if seen_cold and is_fixed:
            pass
        return is_fixed, is_hot

    def parse_import(self) -> ast.ModuleImport:
        tok = self.expect("KEYWORD")
        if tok.value != "takepkg":
            raise ParserError("Expected takepkg")
        module = self.parse_dotted_name()
        alias = None
        if self.match("="):
            alias = self.expect("IDENT").value
        self.expect(";")
        return ast.ModuleImport(module=module, alias=alias, line=tok.line, column=tok.column)

    def parse_dotted_name(self) -> str:
        name = self.expect("IDENT").value
        while self.match("."):
            name += "." + self.expect("IDENT").value
        return name

    def parse_decl_or_stmt(self) -> ast.Node:
        save = self.pos
        is_fixed, is_hot = self.parse_modifiers()
        if self.at("KEYWORD") and self.current().value == "func":
            return self.parse_function(is_fixed=is_fixed, is_hot=is_hot)
        if self.at("IDENT") and self.tokens[self.pos + 1].value in {"(", "=", "+=", "-="}:
            name_tok = self.expect("IDENT")
            type_ref = None
            if self.match("("):
                type_ref = self.parse_type_spec_param()
                self.expect(")")
            op = self.expect("=", "+=", "-=")
            value = self.parse_expression()
            self.expect(";")
            if op.value != "=":
                target = ast.Identifier(name=name_tok.value, line=name_tok.line, column=name_tok.column)
                return ast.Assign(target=target, value=value, op=op.value, line=name_tok.line, column=name_tok.column)
            return ast.VarDecl(name=name_tok.value, type_ref=type_ref, value=value, is_hot=is_hot, is_fixed=is_fixed, line=name_tok.line, column=name_tok.column)
        self.pos = save
        return self.parse_statement()

    def parse_type_until(self, closing: str) -> ast.TypeRef:
        if self.match("["):
            options = [self.parse_type_until(",")]
            while self.match(","):
                options.append(self.parse_type_until("," if not self.at("]") else "]"))
            self.expect("]")
            if self.at("IDENT") or (self.at("KEYWORD") and tok.value == "err"):
                return ast.TypeRef(name="union", options=options)
            return ast.TypeRef(name="union", options=options)
        base_name = self.parse_dotted_name()
        args: list[ast.TypeRef] = []
        if self.match("["):
            while not self.at("]"):
                args.append(self.parse_type_until("," if not self.at("]") else "]"))
                if not self.match(","):
                    break
            self.expect("]")
        return ast.TypeRef(name=base_name, args=args)

    def parse_function(self, is_fixed: bool = False, is_hot: bool = False) -> ast.FunctionDecl:
        func_tok = self.expect("KEYWORD")
        if func_tok.value != "func":
            raise ParserError("Expected func")
        name = self.expect("IDENT")
        params: list[ast.Param] = []
        if self.match("("):
            while not self.at(")"):
                if self.match(";"):
                    continue
                type_ref = self.parse_type_spec_param()
                param_name = self.expect("IDENT")
                params.append(ast.Param(name=param_name.value, type_ref=type_ref, line=param_name.line, column=param_name.column))
                self.match(";")
            self.expect(")")
        if self.match("=>"):
            expr = self.parse_expression()
            self.expect(";")
            return ast.FunctionDecl(name=name.value, params=params, expr_body=expr, is_hot=is_hot, is_fixed=is_fixed, line=func_tok.line, column=func_tok.column)
        body = self.parse_block()
        self.match(";")
        return ast.FunctionDecl(name=name.value, params=params, body=body, is_hot=is_hot, is_fixed=is_fixed, line=func_tok.line, column=func_tok.column)

    def parse_type_spec_param(self) -> ast.TypeRef:
        if self.at("["):
            self.expect("[")
            options = [self.parse_type_spec_param()]
            while self.match(","):
                options.append(self.parse_type_spec_param())
            self.expect("]")
            return ast.TypeRef(name="union", options=options)
        tok_name = self.parse_dotted_name()
        args = []
        if self.match("["):
            while not self.at("]"):
                args.append(self.parse_type_spec_param())
                if not self.match(","):
                    break
            self.expect("]")
        return ast.TypeRef(name=tok_name, args=args)

    def parse_family(self) -> ast.FamilyDecl:
        tok = self.expect("KEYWORD")
        name = self.expect("IDENT")
        base = None
        if self.match("("):
            base = self.expect("IDENT").value
            self.expect(")")
        self.expect("{")
        body = []
        while not self.at("}"):
            if self.peek_decl_start() or (self.at("KEYWORD") and self.current().value in {"func", "family"}):
                body.append(self.parse_toplevel())
            else:
                body.append(self.parse_statement())
        self.expect("}")
        self.match(";")
        return ast.FamilyDecl(name=name.value, base_name=base, body=body, line=tok.line, column=tok.column)

    def parse_block(self) -> ast.Block:
        left = self.expect("{")
        statements = []
        while not self.at("}"):
            if self.peek_decl_start() or (self.at("KEYWORD") and self.current().value in {"func", "family"}):
                statements.append(self.parse_toplevel())
            else:
                statements.append(self.parse_statement())
        self.expect("}")
        return ast.Block(statements=statements, line=left.line, column=left.column)

    def parse_statement(self) -> ast.Node:
        if self.at("KEYWORD"):
            kw = self.current().value
            if kw == "ret":
                tok = self.expect("KEYWORD")
                value = None if self.at(";") else self.parse_expression()
                self.expect(";")
                return ast.Return(value=value, line=tok.line, column=tok.column)
            if kw == "if":
                return self.parse_if()
            if kw == "repeat":
                return self.parse_repeat()
            if kw == "for":
                return self.parse_for()
            if kw == "match":
                return self.parse_match()
            if kw == "run":
                return self.parse_try()
            if kw == "func":
                return self.parse_function()
            if kw == "family":
                return self.parse_family()
        expr = self.parse_expression()
        if isinstance(expr, (ast.Identifier, ast.Attr)) and self.at(*ASSIGN_OPS):
            op = self.expect(*ASSIGN_OPS)
            value = self.parse_expression()
            self.expect(";")
            return ast.Assign(target=expr, value=value, op=op.value, line=expr.line, column=expr.column)
        self.expect(";")
        return ast.ExprStmt(expr=expr, line=expr.line, column=expr.column)

    def parse_if(self) -> ast.IfStmt:
        tok = self.expect("KEYWORD")
        self.expect("(")
        cond = self.parse_expression()
        self.expect(")")
        then_block = self.parse_block()
        elif_blocks = []
        while self.at("KEYWORD") and self.current().value == "elif":
            self.expect("KEYWORD")
            self.expect("(")
            econd = self.parse_expression()
            self.expect(")")
            elif_blocks.append((econd, self.parse_block()))
        else_block = None
        if self.at("KEYWORD") and self.current().value == "else":
            self.expect("KEYWORD")
            else_block = self.parse_block()
        return ast.IfStmt(condition=cond, then_block=then_block, elif_blocks=elif_blocks, else_block=else_block, line=tok.line, column=tok.column)

    def parse_repeat(self) -> ast.RepeatStmt:
        tok = self.expect("KEYWORD")
        count = self.parse_expression()
        body = self.parse_block()
        return ast.RepeatStmt(count=count, body=body, line=tok.line, column=tok.column)

    def parse_for(self) -> ast.ForStmt:
        tok = self.expect("KEYWORD")
        self.expect("(")
        init = self.parse_for_list(stop_at=",")
        self.expect(",")
        cond = self.parse_expression()
        self.expect(",")
        update = self.parse_for_list(stop_at=")")
        self.expect(")")
        body = self.parse_block()
        return ast.ForStmt(init=init, condition=cond, update=update, body=body, line=tok.line, column=tok.column)

    def parse_for_list(self, stop_at: str) -> list[ast.Node]:
        exprs = []
        if self.match("("):
            while not self.at(")"):
                exprs.append(self.parse_for_component())
                if self.at("KEYWORD") and self.current().value == "also":
                    self.expect("KEYWORD")
                else:
                    break
            self.expect(")")
            return exprs
        if not self.at(stop_at):
            exprs.append(self.parse_for_component())
        return exprs

    def parse_for_component(self) -> ast.Node:
        expr = self.parse_expression()
        if isinstance(expr, (ast.Identifier, ast.Attr)) and self.at(*ASSIGN_OPS):
            op = self.expect(*ASSIGN_OPS)
            value = self.parse_expression()
            return ast.Assign(target=expr, value=value, op=op.value, line=expr.line, column=expr.column)
        return expr

    def parse_match(self) -> ast.MatchStmt:
        tok = self.expect("KEYWORD")
        subject = self.parse_expression()
        self.expect("{")
        cases = []
        while not self.at("}"):
            is_default = False
            if self.at("__"):
                self.expect("__")
                pattern = None
                is_default = True
            else:
                pattern = self.parse_expression()
            self.expect("=>")
            if self.at("{"):
                block = self.parse_block()
            else:
                stmt = self.parse_expression()
                self.expect(";")
                block = ast.Block(statements=[ast.ExprStmt(expr=stmt)])
                cases.append(ast.MatchCase(pattern=pattern, block=block, is_default=is_default, line=tok.line, column=tok.column))
                continue
            self.expect(";")
            cases.append(ast.MatchCase(pattern=pattern, block=block, is_default=is_default, line=tok.line, column=tok.column))
        self.expect("}")
        return ast.MatchStmt(subject=subject, cases=cases, line=tok.line, column=tok.column)

    def parse_try(self) -> ast.TryStmt:
        tok = self.expect("KEYWORD")
        body = self.parse_block()
        handle_tok = self.expect("KEYWORD")
        if handle_tok.value != "handle":
            raise ParserError("Expected handle after run block")
        self.expect("(")
        handlers = []
        while not self.at(")"):
            handlers.append(self.expect("IDENT").value)
            if not self.match(","):
                break
        self.expect(")")
        handler_block = self.parse_block()
        finally_block = None
        if self.at("KEYWORD") and self.current().value == "then":
            self.expect("KEYWORD")
            finally_block = self.parse_block()
        return ast.TryStmt(body=body, handlers=handlers, handler_block=handler_block, finally_block=finally_block, line=tok.line, column=tok.column)

    def parse_expression(self, min_prec: int = 0) -> ast.Node:
        expr = self.parse_unary()
        while True:
            tok = self.current()
            op = tok.value
            if op not in PRECEDENCE or PRECEDENCE[op] < min_prec:
                break
            self.pos += 1
            next_min = PRECEDENCE[op] + (0 if op == "**" else 1)
            rhs = self.parse_expression(next_min)
            expr = ast.BinaryOp(left=expr, op=op, right=rhs, line=tok.line, column=tok.column)
        return expr

    def parse_unary(self) -> ast.Node:
        if self.at("-", "+") or (self.at("KEYWORD") and self.current().value == "not"):
            tok = self.current()
            self.pos += 1
            return ast.UnaryOp(op=tok.value, operand=self.parse_unary(), line=tok.line, column=tok.column)
        return self.parse_postfix()

    def parse_postfix(self) -> ast.Node:
        expr = self.parse_primary()
        while True:
            if self.match("("):
                args = []
                while not self.at(")"):
                    args.append(self.parse_expression())
                    if not self.match(","):
                        break
                self.expect(")")
                expr = ast.Call(func=expr, args=args, line=expr.line, column=expr.column)
                continue
            if self.match("."):
                name = self.expect("IDENT")
                expr = ast.Attr(obj=expr, name=name.value, line=name.line, column=name.column)
                continue
            if self.match("++"):
                one = ast.Literal(value=1, literal_kind="int", line=expr.line, column=expr.column)
                expr = ast.Assign(target=expr, value=one, op="+=", line=expr.line, column=expr.column)
                continue
            if self.match("--"):
                one = ast.Literal(value=1, literal_kind="int", line=expr.line, column=expr.column)
                expr = ast.Assign(target=expr, value=one, op="-=", line=expr.line, column=expr.column)
                continue
            break
        return expr

    def parse_primary(self) -> ast.Node:
        tok = self.current()
        if self.match("NUMBER"):
            if "." in tok.value:
                return ast.Literal(value=float(tok.value), literal_kind="float", line=tok.line, column=tok.column)
            return ast.Literal(value=int(tok.value), literal_kind="int", line=tok.line, column=tok.column)
        if self.match("STRING"):
            return ast.Literal(value=tok.value, literal_kind="string", line=tok.line, column=tok.column)
        if self.at("KEYWORD") and tok.value in {"true", "false", "fasle", "null"}:
            self.pos += 1
            mapping = {"true": True, "false": False, "fasle": False, "null": None}
            return ast.Literal(value=mapping[tok.value], literal_kind="bool", line=tok.line, column=tok.column)
        if self.at("IDENT") or (self.at("KEYWORD") and tok.value == "err"):
            self.pos += 1
            ident = ast.Identifier(name=tok.value, line=tok.line, column=tok.column)
            if tok.value == "arr" and self.at("("):
                return self.finish_builtin_collection(ast.ArrayLiteral, tok)
            if tok.value == "set" and self.at("("):
                return self.finish_builtin_collection(ast.SetLiteral, tok)
            if tok.value == "pair" and self.at("("):
                self.expect("(")
                first = self.parse_expression()
                self.expect(",")
                second = self.parse_expression()
                self.expect(")")
                return ast.PairLiteral(first=first, second=second, line=tok.line, column=tok.column)
            if tok.value == "range" and self.at("("):
                self.expect("(")
                start = self.parse_expression()
                self.expect(",")
                end = self.parse_expression()
                self.expect(")")
                return ast.RangeExpr(start=start, end=end, line=tok.line, column=tok.column)
            return ident
        if self.match("("):
            expr = self.parse_expression()
            self.expect(")")
            return expr
        raise ParserError(f"Unexpected token {tok.display()}")

    def finish_builtin_collection(self, cls, tok: Token) -> ast.Node:
        self.expect("(")
        items = []
        while not self.at(")"):
            items.append(self.parse_expression())
            if not self.match(","):
                break
        self.expect(")")
        return cls(items=items, line=tok.line, column=tok.column)
