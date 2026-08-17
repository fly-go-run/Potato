#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""Install Potato into the bundled standalone CPython.

The frozen PyInstaller sidecar is an 18s-class cold start. When this site
install is present the desktop shell starts ``python -m potato.tauri.entry``
instead, which is in the 1–4s range.
"""

from __future__ import annotations

import argparse
import subprocess
import sys
from pathlib import Path


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument(
        "--python",
        required=True,
        help="Bundled CPython executable",
    )
    parser.add_argument(
        "--repo",
        required=True,
        help="Potato repository root to install from",
    )
    args = parser.parse_args()
    python = Path(args.python)
    repo = Path(args.repo)
    if not python.is_file():
        print(f"bundled python not found: {python}", file=sys.stderr)
        return 1
    if not (repo / "pyproject.toml").is_file() and not (repo / "setup.py").is_file():
        print(f"Potato project not found at {repo}", file=sys.stderr)
        return 1

    cmd = [
        str(python),
        "-m",
        "pip",
        "install",
        "--upgrade",
        "--disable-pip-version-check",
        str(repo),
    ]
    print("Installing Potato into bundled CPython:")
    print(" ", " ".join(cmd))
    result = subprocess.run(cmd, check=False)
    if result.returncode != 0:
        return result.returncode

    verify = subprocess.run(
        [str(python), "-c", "import potato.tauri.entry"],
        check=False,
    )
    if verify.returncode != 0:
        print("Potato import check failed after install", file=sys.stderr)
        return verify.returncode
    print("Bundled CPython can import potato.tauri.entry")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
