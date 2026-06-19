import ast
import importlib.util
import sys
from pathlib import Path

SRC = Path(__file__).resolve().parents[1] / "src" / "session_state"

# Everything importable without installing anything (stdlib + our own package).
_STDLIB = set(sys.stdlib_module_names) | {"session_state"}


def _imported_top_levels(py_file: Path) -> set[str]:
    tree = ast.parse(py_file.read_text(encoding="utf-8"))
    tops: set[str] = set()
    for node in ast.walk(tree):
        if isinstance(node, ast.Import):
            for n in node.names:
                tops.add(n.name.split(".")[0])
        elif isinstance(node, ast.ImportFrom) and node.level == 0 and node.module:
            tops.add(node.module.split(".")[0])
    return tops


def test_runtime_modules_import_stdlib_only():
    offenders = {}
    for py_file in SRC.glob("*.py"):
        bad = {t for t in _imported_top_levels(py_file) if t not in _STDLIB}
        if bad:
            offenders[py_file.name] = sorted(bad)
    assert not offenders, f"non-stdlib imports found: {offenders}"
