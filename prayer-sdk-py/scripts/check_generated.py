#!/usr/bin/env python3
"""Fail if checked-in generated Python artifacts differ from OpenAPI."""

from __future__ import annotations

import subprocess
import sys
import tempfile
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
SDK = ROOT / "prayer-sdk-py"

with tempfile.TemporaryDirectory() as directory:
    destination = Path(directory)
    subprocess.run([
        sys.executable, str(SDK / "scripts/generate.py"),
        str(ROOT / "prayer-api/openapi/prayer-v1.json"), str(destination),
    ], check=True)
    stale = [name for name in ("models.py", "api.py")
             if (SDK / "src/prayer_sdk/generated" / name).read_bytes()
             != (destination / name).read_bytes()]
if stale:
    raise SystemExit(f"generated Python artifacts are stale: {', '.join(stale)}")

