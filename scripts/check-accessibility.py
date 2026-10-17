#!/usr/bin/env python3
"""accessibility check for this repository.

The logic lives once, in `workspace-governance/scripts/check_concern.py`. This
file exists because the gate binds `scripts/check-accessibility.py` when it is
present, and that file-gated rule is what turns an unbound check into a
checked one. It is a path, not a copy: a checker duplicated into five
repositories is five checkers that drift.
"""
import subprocess
import sys
from pathlib import Path

GOVERNANCE = Path(__file__).resolve().parents[2] / "workspace-governance"
raise SystemExit(subprocess.run(
    [sys.executable, str(GOVERNANCE / "scripts" / "check_concern.py"), "accessibility", str(Path(__file__).resolve().parents[1])],
).returncode)
