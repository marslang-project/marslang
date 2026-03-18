from __future__ import annotations

from dataclasses import dataclass
import builtins
import sys


class Error(Exception):
    pass


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

    def __iter__(self):
        return iter(self._values)

    def __repr__(self):
        return f"MSet({self._values!r})"


@dataclass
class MPair:
    first: object
    second: object


def a(*values):
    return MArray(values)


def s(*values):
    return MSet(values)


def p(first, second):
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


RUNTIME_HEADER = """from marslang.runtime import MArray, MSet, MPair, a, s, p, out, slout, in_ as in_, inln, CHAR_CNVRT, err, Error, UNPACK_ARR\n"""
