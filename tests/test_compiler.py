from __future__ import annotations

from pathlib import Path
import subprocess
import sys
import textwrap

from marslang.compiler import compile_source


def test_compile_hot_inline_and_family():
    source = textwrap.dedent(
        '''
        fixed hot pi (float) = 3.14159;
        func area(float r;) => pi * r * r;

        family Circle{
            func init(float r;){
                me.r = r;
            }

            func size(){
                ret area(me.r);
            }
        }

        func m{
            nums (array[int]) = a(3,1,4,1,5);
            nums.asort();
            out(nums.iget(0));
            c = Circle(5);
            out(c.size());
        }
        '''
    )
    compiled = compile_source(source)
    assert "def area(r):" in compiled
    assert "return ((3.14159 * r) * r)" in compiled or "return (3.14159 * r * r)" in compiled
    assert "class Circle(object):" in compiled
    assert "def __mars_main__():" in compiled
    assert "nums = MArray([3, 1, 4, 1, 5], type_name='int')" in compiled


def test_compile_control_flow_and_match():
    source = textwrap.dedent(
        '''
        func describe(int x;){
            match x{
                1 => { out("one"); };
                range(2, 4) => { out("small"); };
                __ => { out("other"); };
            }
        }
        '''
    )
    compiled = compile_source(source)
    assert "__mars_match_subject = x" in compiled
    assert "if __mars_match_subject == 1:" in compiled
    assert "elif (2 <= __mars_match_subject < 4):" in compiled
    assert "else:" in compiled


def test_cli_compiles_and_runs(tmp_path: Path):
    source = tmp_path / "hello.mrs"
    source.write_text(
        textwrap.dedent(
            '''
            hot x (int) = 5;
            func m{
                out(x);
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
