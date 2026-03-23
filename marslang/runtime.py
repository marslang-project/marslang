from __future__ import annotations

from dataclasses import dataclass
import builtins
import importlib
import sys


class Error(Exception):
    pass


class ReturnSignal(Exception):
    def __init__(self, value):
        self.value = value


class MArray(list):
    def __init__(self, values=(), type_name=None):
        self.type_name = type_name
        super().__init__()
        for value in values:
            self._check(value)
            super().append(value)

    def _check(self, value):
        if self.type_name is None:
            return
        if self.type_name == "int" and not isinstance(value, int):
            raise TypeError("Expected int in array")
        if self.type_name == "string" and not isinstance(value, str):
            raise TypeError("Expected string in array")
        if self.type_name == "float" and not isinstance(value, float):
            raise TypeError("Expected float in array")
        if self.type_name == "char" and not (isinstance(value, str) and len(value) == 1):
            raise TypeError("Expected char in array")
        if self.type_name == "bool" and not isinstance(value, bool):
            raise TypeError("Expected bool in array")

    def add(self, value):
        self._check(value)
        self.append(value)

    def iget(self, idx):
        return self[idx]

    def pop(self):
        return super().pop(0)

    def lpop(self):
        return super().pop()

    def iremove(self, idx):
        return super().pop(idx)

    def get(self, value):
        return self.index(value)

    def rget(self, value):
        for idx in range(len(self) - 1, -1, -1):
            if self[idx] == value:
                return idx
        raise ValueError(value)

    def remove(self, value):
        return super().remove(value)

    def modify(self, idx, val):
        self._check(val)
        self[idx] = val

    def change(self, prev, curr):
        self._check(curr)
        for idx, value in enumerate(self):
            if value == prev:
                self[idx] = curr

    def sort(self):
        super().sort(reverse=True)

    def asort(self):
        super().sort()

    def rev(self):
        self.reverse()

    def slice(self, startidx, endidx):
        return MArray(self[startidx:endidx], type_name=self.type_name)

    def lenslice(self, startidx, length):
        return self.slice(startidx, startidx + length)


class MSet:
    def __init__(self, values=(), type_name=None):
        self.type_name = type_name
        self._values = []
        for value in values:
            self.add(value)

    def _check(self, value):
        if self.type_name is None:
            return
        if self.type_name == "int" and not isinstance(value, int):
            raise TypeError("Expected int in set")
        if self.type_name == "string" and not isinstance(value, str):
            raise TypeError("Expected string in set")
        if self.type_name == "float" and not isinstance(value, float):
            raise TypeError("Expected float in set")
        if self.type_name == "char" and not (isinstance(value, str) and len(value) == 1):
            raise TypeError("Expected char in set")
        if self.type_name == "bool" and not isinstance(value, bool):
            raise TypeError("Expected bool in set")

    def add(self, value):
        self._check(value)
        if value not in self._values:
            self._values.append(value)

    def iget(self, idx):
        return self._values[idx]

    def pop(self):
        return self._values.pop(0)

    def lpop(self):
        return self._values.pop()

    def iremove(self, idx):
        return self._values.pop(idx)

    def get(self, value):
        return self._values.index(value)

    def rget(self, value):
        for idx in range(len(self._values) - 1, -1, -1):
            if self._values[idx] == value:
                return idx
        raise ValueError(value)

    def remove(self, value):
        self._values.remove(value)

    def modify(self, idx, val):
        self._check(val)
        if val in self._values and self._values[idx] != val:
            raise ValueError("duplicate value in set")
        self._values[idx] = val

    def change(self, prev, curr):
        self._check(curr)
        self._values = [curr if v == prev else v for v in self._values]
        self._values = list(dict.fromkeys(self._values))

    def sort(self):
        self._values.sort(reverse=True)

    def asort(self):
        self._values.sort()

    def rev(self):
        self._values.reverse()

    reverse = rev

    def slice(self, startidx, endidx):
        return MArray(self._values[startidx:endidx], type_name=self.type_name)

    def lenslice(self, startidx, length):
        return self.slice(startidx, startidx + length)

    def __getitem__(self, index):
        return self._values[index]

    def __setitem__(self, index, value):
        self.modify(index, value)

    def __iter__(self):
        return iter(self._values)

    def __len__(self):
        return len(self._values)

    def __repr__(self):
        return f"MSet({self._values!r})"


@dataclass
class MPair:
    first: object
    second: object

    def __getitem__(self, index):
        return self.first if index == 0 else self.second

    def __setitem__(self, index, value):
        if index == 0:
            self.first = value
        elif index == 1:
            self.second = value
        else:
            raise IndexError(index)


def arr(*values):
    return MArray(values)


def set(*values):  # noqa: A001 - language builtin name
    return MSet(values)


def pair(first, second):
    return MPair(first, second)


def out(*values):
    print(*values)


def slout(*values):
    print(*values, end="")


def in_():
    return sys.stdin.read()


def inln():
    return sys.stdin.readline().rstrip("\n")


def CHAR_CNVRT(value):
    value = str(value)
    if len(value) != 1:
        raise ValueError("CHAR_CNVRT expects a single character")
    return value


def err(error_type, message):
    if isinstance(error_type, str):
        exc = getattr(builtins, error_type, None) or globals().get(error_type, Error)
    else:
        exc = error_type
    raise exc(message)


def UNPACK_ARR(values):
    return list(values)


class Environment:
    def __init__(self, parent: Environment | None = None):
        self.parent = parent
        self.values: dict[str, object] = {}
        self.constants: set[str] = set()

    def define(self, name: str, value: object, constant: bool = False) -> None:
        self.values[name] = value
        if constant:
            self.constants.add(name)

    def resolve(self, name: str) -> "Environment":
        if name in self.values:
            return self
        if self.parent is not None:
            return self.parent.resolve(name)
        raise NameError(name)

    def get(self, name: str) -> object:
        if name in self.values:
            return self.values[name]
        if self.parent is not None:
            return self.parent.get(name)
        raise NameError(name)

    def assign(self, name: str, value: object) -> None:
        env = self.resolve(name)
        if name in env.constants:
            raise TypeError(f"Cannot reassign constant/hot variable {name}")
        env.values[name] = value


class VirtualMachine:
    def __init__(self, program: dict):
        self.program = program
        self.globals = Environment()
        self.install_builtins()

    def install_builtins(self) -> None:
        builtins_map = {
            "arr": arr,
            "set": set,
            "pair": pair,
            "out": out,
            "slout": slout,
            "in": in_,
            "inln": inln,
            "CHAR_CNVRT": CHAR_CNVRT,
            "err": err,
            "Error": Error,
            "UNPACK_ARR": UNPACK_ARR,
            "int": int,
            "longint": int,
            "string": str,
            "float": float,
        }
        for name, value in builtins_map.items():
            self.globals.define(name, value, constant=True)

    def execute_program(self, invoke_main: bool = True):
        for name in self.program.get("constants", []):
            self.globals.constants.add(name)
        for stmt in self.program["body"]:
            self.exec_stmt(stmt, self.globals)
        if invoke_main:
            try:
                main_fn = self.globals.get("m")
            except NameError:
                return None
            return main_fn()
        return None

    def exec_block(self, body: list[dict], env: Environment):
        for stmt in body:
            self.exec_stmt(stmt, env)

    def exec_stmt(self, stmt: dict, env: Environment):
        kind = stmt["kind"]
        if kind == "Import":
            module = importlib.import_module(stmt["module"])
            env.define(stmt["alias"] or stmt["module"].split(".")[-1], module)
            return None
        if kind == "VarDecl":
            value = self.eval_expr(stmt["value"], env)
            value = self.apply_type_restriction(value, stmt.get("type"))
            env.define(stmt["name"], value, constant=stmt.get("fixed", False))
            return None
        if kind == "FunctionDecl":
            env.define(stmt["name"], self.make_function(stmt, env), constant=stmt.get("fixed", False) or stmt.get("hot", False))
            return None
        if kind == "FamilyDecl":
            env.define(stmt["name"], self.make_family(stmt, env))
            return None
        if kind == "ExprStmt":
            return self.eval_expr(stmt["expr"], env)
        if kind == "Assign":
            return self.exec_assign(stmt, env)
        if kind == "Return":
            raise ReturnSignal(None if stmt["value"] is None else self.eval_expr(stmt["value"], env))
        if kind == "IfStmt":
            if self.eval_expr(stmt["condition"], env):
                self.exec_block(stmt["then"], Environment(env))
                return None
            for branch in stmt["elifs"]:
                if self.eval_expr(branch["condition"], env):
                    self.exec_block(branch["body"], Environment(env))
                    return None
            if stmt["else"]:
                self.exec_block(stmt["else"], Environment(env))
            return None
        if kind == "RepeatStmt":
            for _ in range(self.eval_expr(stmt["count"], env)):
                self.exec_block(stmt["body"], Environment(env))
            return None
        if kind == "ForStmt":
            loop_env = Environment(env)
            for item in stmt["init"]:
                self.exec_stmt(item, loop_env)
            while self.eval_expr(stmt["condition"], loop_env):
                self.exec_block(stmt["body"], Environment(loop_env))
                for item in stmt["update"]:
                    self.exec_stmt(item, loop_env)
            return None
        if kind == "MatchStmt":
            subject = self.eval_expr(stmt["subject"], env)
            for case in stmt["cases"]:
                if case["default"] or self.match_case(subject, case["pattern"], env):
                    self.exec_block(case["body"], Environment(env))
                    break
            return None
        if kind == "TryStmt":
            try:
                self.exec_block(stmt["body"], Environment(env))
            except tuple(self.resolve_error(name, env) for name in stmt["handlers"] or ["Error"]):
                self.exec_block(stmt["handler_body"], Environment(env))
            finally:
                if stmt["then_body"]:
                    self.exec_block(stmt["then_body"], Environment(env))
            return None
        raise RuntimeError(f"Unknown statement kind {kind}")

    def resolve_error(self, name: str, env: Environment):
        try:
            value = env.get(name)
        except NameError:
            value = getattr(builtins, name, None) or globals().get(name, Error)
        return value

    def exec_assign(self, stmt: dict, env: Environment):
        target = stmt["target"]
        value = self.eval_expr(stmt["value"], env)
        if target["kind"] == "Identifier":
            name = target["name"]
            current = env.get(name) if stmt["op"] != "=" else None
            if stmt["op"] == "+=":
                value = current + value
            elif stmt["op"] == "-=":
                value = current - value
            if stmt["op"] == "=":
                try:
                    env.assign(name, value)
                except NameError:
                    env.define(name, value)
            else:
                env.assign(name, value)
            return value
        if target["kind"] == "Attr":
            obj = self.eval_expr(target["obj"], env)
            if stmt["op"] != "=":
                current = getattr(obj, target["name"])
                value = current + value if stmt["op"] == "+=" else current - value
            setattr(obj, target["name"], value)
            return value
        if target["kind"] == "Index":
            obj = self.eval_expr(target["obj"], env)
            index = self.eval_expr(target["index"], env)
            if stmt["op"] != "=":
                current = obj[index]
                value = current + value if stmt["op"] == "+=" else current - value
            obj[index] = value
            return value
        raise RuntimeError(f"Unsupported assign target {target['kind']}")

    def make_function(self, stmt: dict, defining_env: Environment):
        def fn(*args):
            local_env = Environment(defining_env)
            for param, arg in zip(stmt["params"], args):
                local_env.define(param["name"], self.apply_type_restriction(arg, param.get("type")))
            try:
                self.exec_block(stmt["body"], local_env)
            except ReturnSignal as signal:
                return signal.value
            return None

        fn.__name__ = stmt["name"]
        return fn

    def make_family(self, stmt: dict, defining_env: Environment):
        base = object
        if stmt.get("base"):
            base = defining_env.get(stmt["base"])
        attrs = {}
        for item in stmt["body"]:
            if item["kind"] == "FunctionDecl":
                attrs["__init__" if item["name"] == "init" else item["name"]] = self.make_method(item, defining_env)
            elif item["kind"] == "VarDecl":
                attrs[item["name"]] = self.apply_type_restriction(self.eval_expr(item["value"], defining_env), item.get("type"))
        return type(stmt["name"], (base,), attrs)

    def make_method(self, stmt: dict, defining_env: Environment):
        def method(self_obj, *args):
            local_env = Environment(defining_env)
            local_env.define("me", self_obj)
            local_env.define("self", self_obj)
            for param, arg in zip(stmt["params"], args):
                local_env.define(param["name"], self.apply_type_restriction(arg, param.get("type")))
            try:
                self.exec_block(stmt["body"], local_env)
            except ReturnSignal as signal:
                return signal.value
            return None

        method.__name__ = stmt["name"]
        return method

    def match_case(self, subject, pattern: dict | None, env: Environment) -> bool:
        if pattern is None:
            return True
        if pattern["kind"] == "RangeExpr":
            start = self.eval_expr(pattern["start"], env)
            end = self.eval_expr(pattern["end"], env)
            return start <= subject < end
        return subject == self.eval_expr(pattern, env)

    def eval_expr(self, expr: dict, env: Environment):
        kind = expr["kind"]
        if kind == "Literal":
            return expr["value"]
        if kind == "Identifier":
            return env.get(expr["name"])
        if kind == "ArrayLiteral":
            values = [self.eval_expr(item, env) for item in expr["items"]]
            value = MArray(values, type_name=self.container_type(expr.get("type")))
            return self.apply_type_restriction(value, expr.get("type"))
        if kind == "SetLiteral":
            values = [self.eval_expr(item, env) for item in expr["items"]]
            value = MSet(values, type_name=self.container_type(expr.get("type")))
            return self.apply_type_restriction(value, expr.get("type"))
        if kind == "PairLiteral":
            value = MPair(self.eval_expr(expr["first"], env), self.eval_expr(expr["second"], env))
            return self.apply_type_restriction(value, expr.get("type"))
        if kind == "UnaryOp":
            value = self.eval_expr(expr["operand"], env)
            if expr["op"] == "-":
                return -value
            if expr["op"] == "+":
                return +value
            if expr["op"] == "not":
                return not value
        if kind == "BinaryOp":
            if expr["op"] == "and":
                return self.eval_expr(expr["left"], env) and self.eval_expr(expr["right"], env)
            if expr["op"] in {"or", "and/or"}:
                return self.eval_expr(expr["left"], env) or self.eval_expr(expr["right"], env)
            left = self.eval_expr(expr["left"], env)
            right = self.eval_expr(expr["right"], env)
            return self.apply_binary(expr["op"], left, right)
        if kind == "Call":
            func = self.eval_expr(expr["func"], env)
            args = [self.eval_expr(arg, env) for arg in expr["args"]]
            return func(*args)
        if kind == "Attr":
            obj = self.eval_expr(expr["obj"], env)
            return getattr(obj, expr["name"])
        if kind == "Index":
            obj = self.eval_expr(expr["obj"], env)
            index = self.eval_expr(expr["index"], env)
            return obj[index]
        if kind == "Assign":
            return self.exec_assign(expr, env)
        if kind == "RangeExpr":
            return range(self.eval_expr(expr["start"], env), self.eval_expr(expr["end"], env))
        raise RuntimeError(f"Unknown expression kind {kind}")

    def apply_binary(self, op: str, left, right):
        if op == "+":
            return left + right
        if op == "-":
            return left - right
        if op == "*":
            return left * right
        if op == "/":
            return left / right
        if op == "%":
            return left % right
        if op == "**":
            return left ** right
        if op == "==":
            return left == right
        if op == "!=":
            return left != right
        if op == "<":
            return left < right
        if op == "<=":
            return left <= right
        if op == ">":
            return left > right
        if op == ">=":
            return left >= right
        raise RuntimeError(f"Unsupported operator {op}")

    def container_type(self, type_ref: dict | None):
        if type_ref and type_ref["name"] in {"array", "set"} and type_ref["args"]:
            inner = type_ref["args"][0]
            if inner["name"] != "union":
                return inner["name"]
        return None

    def type_repr(self, type_ref: dict | None) -> str:
        if type_ref is None:
            return "any"
        if type_ref["name"] == "union":
            return "[" + ", ".join(self.type_repr(opt) for opt in type_ref["options"]) + "]"
        if type_ref["args"]:
            return f"{type_ref['name']}[" + ", ".join(self.type_repr(arg) for arg in type_ref["args"]) + "]"
        return type_ref["name"]

    def matches_type(self, value, type_ref: dict | None) -> bool:
        if type_ref is None:
            return True
        if type_ref["name"] == "union":
            return any(self.matches_type(value, option) for option in type_ref["options"])
        name = type_ref["name"]
        if name in {"int", "longint"}:
            return isinstance(value, int) and not isinstance(value, bool)
        if name == "float":
            return isinstance(value, (int, float)) and not isinstance(value, bool)
        if name == "string":
            return isinstance(value, str)
        if name == "char":
            return isinstance(value, str) and len(value) == 1
        if name == "bool":
            return isinstance(value, bool)
        if name == "null":
            return value is None
        if name == "array":
            return isinstance(value, MArray) and (
                not type_ref["args"] or all(self.matches_type(item, type_ref["args"][0]) for item in value)
            )
        if name == "set":
            return isinstance(value, MSet) and (
                not type_ref["args"] or all(self.matches_type(item, type_ref["args"][0]) for item in value)
            )
        if name == "pair":
            if not isinstance(value, MPair):
                return False
            if not type_ref["args"]:
                return True
            if len(type_ref["args"]) != 2:
                return False
            return self.matches_type(value.first, type_ref["args"][0]) and self.matches_type(value.second, type_ref["args"][1])
        return True

    def apply_type_restriction(self, value, type_ref: dict | None):
        if type_ref is None:
            return value
        if not self.matches_type(value, type_ref):
            raise TypeError(f"Value {value!r} does not satisfy type {self.type_repr(type_ref)}")
        if type_ref["name"] == "float" and isinstance(value, int) and not isinstance(value, bool):
            return float(value)
        if type_ref["name"] == "array" and isinstance(value, MArray):
            value.type_name = self.container_type(type_ref)
        if type_ref["name"] == "set" and isinstance(value, MSet):
            value.type_name = self.container_type(type_ref)
        return value


def execute_program(program: dict, invoke_main: bool = True):
    return VirtualMachine(program).execute_program(invoke_main=invoke_main)
