import json
import shutil
import subprocess
from pathlib import Path

import pytest

TOOL = Path(__file__).resolve().parents[1]


def _pwsh():
    return shutil.which("pwsh") or shutil.which("pwsh.exe")


@pytest.mark.skipif(_pwsh() is None, reason="pwsh not available")
def test_print_hooks_only_shape():
    out = subprocess.run([_pwsh(), "-NoProfile", "-File", str(TOOL / "install.ps1"), "-PrintHooksOnly"],
                         capture_output=True, text=True)
    data = json.loads(out.stdout)
    assert set(data) == {"SessionStart", "Stop", "SessionEnd"}
    stop = data["Stop"][0]["hooks"][0]
    assert stop["timeout"] == 5
    assert "capture_scratch.py" in stop["command"]


@pytest.mark.skipif(_pwsh() is None, reason="pwsh not available")
def test_idempotent_against_recall_fixture(tmp_path):
    # fixture settings.json already containing a recall.py SessionStart hook
    settings = tmp_path / "settings.json"
    settings.write_text(json.dumps({"hooks": {"SessionStart": [
        {"hooks": [{"type": "command", "command": '"python.exe" "C:/x/recall.py"'}]}]}}), encoding="utf-8")
    install = [_pwsh(), "-NoProfile", "-File", str(TOOL / "install.ps1"),
               "-SettingsPath", str(settings), "-InstallDir", str(tmp_path / "tool")]
    subprocess.run(install, capture_output=True, text=True)
    subprocess.run(install, capture_output=True, text=True)  # run twice
    data = json.loads(settings.read_text(encoding="utf-8"))
    starts = data["hooks"]["SessionStart"]
    # recall.py entry preserved; exactly ONE of ours (no duplicate on re-run)
    cmds = [h["command"] for e in starts for h in e["hooks"]]
    assert any("recall.py" in c for c in cmds)
    assert sum("resume.py" in c for c in cmds) == 1
