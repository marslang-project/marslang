from __future__ import annotations

from contextlib import redirect_stdout
from io import StringIO
from pathlib import Path
import subprocess
import sys
import textwrap

from marslang.compiler import compile_file_detailed, compile_source
from marslang.runtime import execute_program


def run_compiled_source(source: str) -> tuple[str, dict]:
    compiled = compile_source(source)
    namespace: dict = {"__name__": "compiled_test_module"}
    exec(compiled, namespace)
    stdout = StringIO()
    with redirect_stdout(stdout):
        execute_program(namespace["PROGRAM"])
    return stdout.getvalue(), namespace["PROGRAM"]


def test_vm_backend_executes_hot_inline_and_family_program():
    source = textwrap.dedent(
        '''
        fixed hot pi (float) = 3.14159;
        hot func area(float r;) => pi * r * r;

        family Circle{
            func init(float r;){
                me.r = r;
            }

            func size(){
                ret area(me.r);
            }
        }

        func m{
            nums (array[int]) = arr(3,1,4,1,5);
            nums.asort();
            out(nums.iget(0));
            c = Circle(5);
            out(c.size());
        }
        '''
    )
    output, program = run_compiled_source(source)
    assert output.splitlines() == ["1", "78.53975"]
    assert program["kind"] == "Program"
    assert any(item["kind"] == "FunctionDecl" and item["name"] == "area" for item in program["body"])
    assert "pi" in program["constants"]


def test_then_match_for_index_and_union_type_semantics():
    source = textwrap.dedent(
        '''
        func describe([int, string] x;){
            match x{
                1 => { out("one"); };
                __ => { out("other"); };
            }
        }

        func m{
            total = 0;
            nums (array[int]) = arr(10, 20, 30);
            nums[1] = 25;
            for(i = 0, i < 3, i++){
                total += i;
            }
            run{
                err(Error, "boom");
            } handle(Error){
                out(total);
                out(nums[1]);
            } then{
                describe("x");
            }
        }
        '''
    )
    output, _ = run_compiled_source(source)
    assert output.splitlines() == ["3", "25", "other"]


def test_compile_file_detailed_reports_counts(tmp_path: Path):
    source = tmp_path / "report.mrs"
    source.write_text(
        textwrap.dedent(
            '''
            hot x (int) = 5;
            func helper(int y;) => x + y;
            func m{
                out(helper(2));
            }
            '''
        ),
        encoding="utf-8",
    )
    result = compile_file_detailed(source)
    assert result.output_path == source.with_suffix(".py")
    assert result.token_count > 0
    assert result.top_level_count == 3
    assert "execute_program(PROGRAM)" in result.python_source


def test_cli_compiles_and_runs_vm_output_with_verbose(tmp_path: Path):
    source = tmp_path / "hello.mrs"
    source.write_text(
        textwrap.dedent(
            '''
            hot x (int) = 5;
            func m{
                nums = arr(x, 6);
                out(nums.iget(0));
            }
            '''
        ),
        encoding="utf-8",
    )
    result = subprocess.run(
        [sys.executable, "-m", "marslang.cli", str(source), "--run", "--verbose"],
        capture_output=True,
        text=True,
        check=False,
    )
    assert result.returncode == 0, result.stderr
    assert "Compiled" in result.stdout
    assert "[marslang] lexed" in result.stdout
    assert "[marslang] parsed" in result.stdout
    assert "[marslang] summary:" in result.stdout
    assert result.stdout.rstrip().endswith("5")
    compiled = source.with_suffix(".py").read_text(encoding="utf-8")
    assert "execute_program(PROGRAM)" in compiled


def test_cli_runtime_errors_are_user_friendly(tmp_path: Path):
    source = tmp_path / "boom.mrs"
    source.write_text(
        textwrap.dedent(
            '''
            func m{
                err(Error, "boom");
            }
            '''
        ),
        encoding="utf-8",
    )
    result = subprocess.run(
        [sys.executable, "-m", "marslang.cli", str(source), "--run"],
        capture_output=True,
        text=True,
        check=False,
    )
    assert result.returncode == 1
    assert "marslang runtime error: boom" in result.stderr
    assert "Traceback" not in result.stderr


def test_wrapper_scripts_exist_for_unix_and_windows():
    assert Path("bin/compiler").exists()
    assert Path("bin/compiler.bat").exists()
