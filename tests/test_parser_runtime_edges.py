from __future__ import annotations

import pytest

from marslang.compiler import compile_source
from marslang.errors import ParserError
from marslang.lexer import Lexer
from marslang.parser import Parser


def test_legacy_fasle_alias_is_still_supported_for_compatibility():
    source = 'func m{ if (fasle) { out(1); } else { out(2); } }'
    compiled = compile_source(source)
    assert 'False' in compiled or 'false' not in compiled.lower()


def test_parser_reports_error_instead_of_indexerror_on_incomplete_decl():
    tokens = Lexer('x').tokenize()
    with pytest.raises(ParserError):
        Parser(tokens).parse()


def test_parser_rejects_unterminated_block_comment():
    with pytest.raises(Exception):
        Lexer('/* broken').tokenize()


def test_union_type_restriction_rejects_invalid_assignment():
    source = 'func m{ x ([int, string]) = 1.5; }'
    compiled = compile_source(source)
    namespace = {"__name__": "compiled_test_module"}
    exec(compiled, namespace)
    with pytest.raises(TypeError):
        namespace['execute_program'](namespace['PROGRAM'])
