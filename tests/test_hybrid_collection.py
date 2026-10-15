"""Check hybrid collection parity and report lifecycle against pytest."""

import os
import subprocess
import sys
import textwrap
from pathlib import Path

from test_helpers import create_test_project, extract_test_lines

from test_utils import RtestResult, run_rtest


def pytest_collect(root: Path, files: list[str]) -> list[str]:
    """Use the installed pytest as the oracle, optionally testing another version."""
    result = subprocess.run(
        [
            os.environ.get("RTEST_PARITY_PYTHON", sys.executable),
            "-m",
            "pytest",
            "--collect-only",
            "-q",
            "-p",
            "no:cacheprovider",
            *files,
        ],
        cwd=root,
        env={**os.environ, "PYTEST_DISABLE_PLUGIN_AUTOLOAD": "1"},
        capture_output=True,
        text=True,
    )
    assert result.returncode == 0, result.stdout + result.stderr
    return [line for line in result.stdout.splitlines() if line.startswith(tuple(files)) and "::" in line]


def collect(root: Path, *args: str) -> RtestResult:
    return run_rtest(["--collect-only", *args], cwd=str(root))


def normalize_nodeid(nodeid: str) -> str:
    path, separator, test = nodeid.partition("::")
    return path.replace("\\", "/") + separator + test


def test_hybrid_collection_matches_pytest() -> None:
    files = {
        "test_static.py": """
            import pytest
            @pytest.mark.parametrize("x", [1, 2])
            class TestClass:
                @pytest.mark.parametrize("y", ["a", "b"])
                def test_combined(self, x, y): pass
            def helper(): return 1
            @pytest.mark.parametrize("value", [helper(), helper()], ids=["one", "two"])
            def test_explicit_ids(value): pass
            @pytest.mark.parametrize("value", [pytest.param(helper(), id="caf\u00e9")])
            def test_escaped_id(value): pass
        """,
        "test_duplicates.py": """
            import pytest
            @pytest.mark.parametrize("value", [1, 1])
            def test_numeric(value): pass
            @pytest.mark.parametrize("value", [1, 2, 3], ids=["a", "a", "a0"])
            def test_collision(value): pass
            @pytest.mark.parametrize("x", [1, 2])
            @pytest.mark.parametrize("y", ["a", "a"])
            def test_stacked(x, y): pass
        """,
        "test_objects.py": """
            import pytest
            from dataclasses import dataclass
            @dataclass
            class Row:
                value: int
            @pytest.mark.parametrize("value", [Row(1), Row(2)])
            def test_objects(value): pass
            @pytest.mark.parametrize("x,y", [(str(1), str(2))])
            def test_helper_calls(x, y): pass
            def make_row(): return (1, 2)
            @pytest.mark.parametrize("x,y", [make_row()], ids=["row"])
            def test_opaque_row(x, y): pass
            @pytest.mark.parametrize("value", [1, 2], ids=[None, "named"])
            def test_partial_ids(value): pass
            @pytest.mark.parametrize("value", [1, 2], ids=str)
            def test_callable_ids(value): pass
            VALUES = [1, 2]
            @pytest.mark.parametrize("value", [*VALUES], ids=["one", "two"])
            def test_unpacked(value): pass
            @pytest.mark.parametrize("value", [*VALUES])
            def test_unpacked_without_ids(value): pass
            @pytest.mark.parametrize("value", [])
            def test_empty(value): pass
        """,
        "test_fixture.py": """
            from pytest import fixture as fx, mark
            @fx(params=[1, 2])
            async def value(request): return request.param
            @mark.parametrize("other", [3, 4])
            def test_combined(value, other): pass
        """,
        "base.py": """
            import pytest
            class Base:
                @pytest.fixture(params=[1, 2], autouse=True)
                def mode(self, request): pass
        """,
        "test_inherited.py": """
            from base import Base
            class TestChild(Base):
                def test_item(self): pass
        """,
        "fixtures/conftest.py": """
            import pytest as pt
            @pt.fixture(params=[1, 2], autouse=True)
            def value(request): return request.param
        """,
        "fixtures/nested/test_autouse.py": "def test_item(): pass\n",
        "generated/conftest.py": """
            def pytest_generate_tests(metafunc):
                metafunc.parametrize("value", [1, 2])
        """,
        "generated/test_hook.py": "def test_item(value): pass\n",
        "test_module_mark.py": """
            import pytest
            pytestmark = pytest.mark.parametrize("value", [1, 2])
            def test_item(value): pass
        """,
        "test_class_mark.py": """
            import pytest
            class TestMarked:
                pytestmark = pytest.mark.parametrize("value", [1, 2])
                def test_item(self, value): pass
        """,
    }
    with create_test_project({name: textwrap.dedent(source) for name, source in files.items()}) as root:
        test_files = [name for name in files if Path(name).name.startswith("test_")]
        report = root / "fallback.txt"
        result = collect(root, "--cannot-expand-report", str(report), *test_files)
        assert result.returncode == 0, result.output
        assert "rtest-cannot-expand:" not in result.stdout
        reported = {
            normalize_nodeid(line.removeprefix("rtest-cannot-expand: ")) for line in report.read_text().splitlines()
        }
        fallback = {nodeid.split("::", 1)[0] for nodeid in reported}
        assert fallback == set(test_files) - {"test_static.py"}
        collected = [normalize_nodeid(line) for line in extract_test_lines(result.output_lines)]
        assert reported == {nodeid for nodeid in collected if nodeid.split("::", 1)[0] in fallback}
        static = [nodeid for nodeid in collected if nodeid not in reported]
        hybrid = static + pytest_collect(root, sorted(fallback))
        assert sorted(hybrid) == sorted(pytest_collect(root, test_files))
        # Compare multisets: pytest 7 itself can emit duplicates for colliding explicit IDs.


def test_cannot_expand_report_destinations_and_lifecycle() -> None:
    files = {
        "test_dynamic.py": "import pytest\n@pytest.mark.parametrize('x', range(2))\ndef test_collected(x): pass\n",
        "test_static.py": "def test_static(): pass\n",
        "test_empty.py": "",
        "test_bad.py": "def test_broken(\n",
    }
    with create_test_project(files) as root:
        record = "rtest-cannot-expand: test_dynamic.py::test_collected"
        for destination in ("stdout", "stderr", "none"):
            result = collect(root, "--cannot-expand-report", destination, "test_dynamic.py")
            assert result.returncode == 0, result.output
            assert (record in result.stdout) == (destination == "stdout")
            assert (record in result.stderr) == (destination == "stderr")
        assert record in collect(root, "test_dynamic.py").stdout

        report = root / "fallback.txt"
        args = ("--cannot-expand-report", str(report))
        result = collect(root, *args, "test_dynamic.py")
        assert result.returncode == 0, result.output
        assert report.read_text().splitlines() == [record]
        for name, code in (("test_static.py", 0), ("test_empty.py", 0), ("missing.py", 4), ("test_bad.py", 1)):
            report.write_text("stale report\n")
            result = collect(root, *args, name)
            assert result.returncode == code, result.output
            assert report.read_text() == ""

        result = collect(root, *args, "test_dynamic.py", "test_bad.py")
        assert result.returncode == 1, "Partial collection must not be accepted as success"
        assert "test_dynamic.py::test_collected" in result.stdout
        assert "ERRORS" in result.output and "test_bad.py" in result.output
        assert report.read_text().splitlines() == [record]
        result = collect(root, "--cannot-expand-report", str(root / "missing" / "report.txt"), "test_dynamic.py")
        assert result.returncode != 0
        assert "cannot open cannot-expand report" in result.stderr
