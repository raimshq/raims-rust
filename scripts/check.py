#!/usr/bin/env python3
"""Run the canonical Rust SDK engineering validation."""
from __future__ import annotations

import subprocess
import sys
from pathlib import Path

if sys.flags.optimize or sys.version_info < (3, 12):
    raise SystemExit("Complete Rust SDK validation requires Python 3.12 or later without -O/-OO. Offline generation supports Python 3.8 or later.")

ROOT = Path(__file__).resolve().parent.parent
WORKSPACE = ROOT.parent
VALIDATOR = WORKSPACE / "Developer-Toolkit/scripts/release-preflight/rust_sdk.py"

if not VALIDATOR.is_file() or not (WORKSPACE / "AGENTS.md").is_file():
    raise SystemExit("Validation requires the Raims engineering workspace and its sibling Developer-Toolkit checkout. No checks were run.")
raise SystemExit(subprocess.call([sys.executable, str(VALIDATOR), "--source", str(ROOT), "--workspace", str(WORKSPACE), *sys.argv[1:]]))
