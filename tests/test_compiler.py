from __future__ import annotations

from contextlib import redirect_stdout
from io import StringIO
from pathlib import Path
import subprocess
import sys
import textwrap

from marslang.compiler import compile_source
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


def test_then_and_match_and_for_loop_semantics():
    source = textwrap.dedent(
        '''
        func describe(int x;){
            match x{
                1 => { out("one"); };
                range(2, 4) => { out("small"); };
                __ => { out("other"); };
            }
        }

        func m{
            total = 0;
            for(i = 0, i < 3, i++){
                total += i;
            }
            run{
                err(Error, "boom");
            } handle(Error){
                out(total);
            } then{
                describe(3);
            }
        }
        '''
    )
    output, _ = run_compiled_source(source)
    assert output.splitlines() == ["3", "small"]


def test_cli_compiles_and_runs_vm_output(tmp_path: Path):
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
        )
    )
    result = subprocess.run(
        [sys.executable, "-m", "marslang.cli", str(source), "--run"],
        capture_output=True,
        text=True,
        check=False,
    )
    assert result.returncode == 0, result.stderr
    assert "Compiled" in result.stdout
    assert result.stdout.rstrip().endswith("5")
    compiled = source.with_suffix(".py").read_text()
    assert "execute_program(PROGRAM)" in compiled
