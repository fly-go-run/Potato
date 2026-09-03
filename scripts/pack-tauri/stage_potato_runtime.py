#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""Install Potato into the bundled standalone CPython.

The frozen PyInstaller sidecar is an 18s-class cold start. When this site
install is present the desktop shell starts ``python -m potato.tauri.entry``
instead, which is in the 1–4s range.

On Windows the python-build-standalone runtime ships ``vcruntime140.dll``
but not the C++ runtime (``msvcp140.dll``). C++ extension modules such as
``ujson`` and ``lxml`` fail to load on machines without the x64 VC++
Redistributable, which crashes the backend at startup. PyInstaller copies
these DLLs next to the executable; do the same here (Microsoft permits
redistributing the VC++ runtime DLLs with an application).
"""

from __future__ import annotations

import argparse
import glob
import os
import shutil
import subprocess
import sys
from pathlib import Path

# Runtime DLLs the bundled CPython must carry on Windows. msvcp140.dll is
# mandatory (C++ extensions); the rest are copied when available.
WINDOWS_CRT_REQUIRED = ("msvcp140.dll",)
WINDOWS_CRT_OPTIONAL = (
    "vcruntime140.dll",
    "vcruntime140_1.dll",
    "msvcp140_1.dll",
    "msvcp140_2.dll",
    "msvcp140_atomic_wait.dll",
    "msvcp140_codecvt_ids.dll",
    "concrt140.dll",
)

# Extension modules that must import inside the bundled runtime. ujson and
# lxml are C++ (need msvcp140.dll); orjson/pydantic_core are Rust/C and
# catch a generally broken site install.
NATIVE_IMPORT_CHECK = (
    "ujson",
    "lxml.etree",
    "orjson",
    "pydantic_core",
)


def _windows_crt_source_dirs() -> list[Path]:
    """Candidate directories holding the x64 VC++ runtime DLLs."""
    candidates: list[Path] = []
    redist = os.environ.get("VCToolsRedistDir", "").strip()
    if redist:
        candidates.extend(
            Path(p)
            for p in glob.glob(
                os.path.join(redist, "x64", "Microsoft.VC*.CRT"),
            )
        )
    for root in (
        os.environ.get("ProgramFiles", r"C:\Program Files"),
        os.environ.get("ProgramFiles(x86)", r"C:\Program Files (x86)"),
    ):
        pattern = os.path.join(
            root,
            "Microsoft Visual Studio",
            "*",
            "*",
            "VC",
            "Redist",
            "MSVC",
            "*",
            "x64",
            "Microsoft.VC*.CRT",
        )
        candidates.extend(
            Path(p) for p in sorted(glob.glob(pattern), reverse=True)
        )
    system_root = os.environ.get("SystemRoot", r"C:\Windows")
    candidates.append(Path(system_root) / "System32")
    return [c for c in candidates if c.is_dir()]


def stage_windows_crt(python: Path) -> int:
    """Copy VC++ runtime DLLs next to python.exe. Returns 0 on success."""
    if os.name != "nt":
        return 0
    dest = python.parent
    sources = _windows_crt_source_dirs()
    print("Staging VC++ runtime DLLs into bundled CPython:")
    for name in WINDOWS_CRT_REQUIRED + WINDOWS_CRT_OPTIONAL:
        target = dest / name
        if target.is_file():
            print(f"  present  {name}")
            continue
        found = next((d / name for d in sources if (d / name).is_file()), None)
        if found is None:
            if name in WINDOWS_CRT_REQUIRED:
                print(
                    f"  MISSING  {name}: not found in any of "
                    f"{[str(d) for d in sources]}",
                    file=sys.stderr,
                )
                return 1
            print(f"  skip     {name} (not available on build machine)")
            continue
        shutil.copy2(found, target)
        print(f"  copied   {name} <- {found}")
    return 0


def verify_native_extensions(python: Path) -> int:
    """Import the native extensions that the desktop backend needs."""
    # Every installed module must import; an absent package is a dependency
    # question, not a runtime DLL failure, so it is reported and skipped.
    code = (
        "import importlib, importlib.util, sys\n"
        f"names = {list(NATIVE_IMPORT_CHECK)!r}\n"
        "failed = []\n"
        "for name in names:\n"
        "    try:\n"
        "        spec = importlib.util.find_spec(name)\n"
        "    except ModuleNotFoundError:\n"
        "        spec = None\n"
        "    if spec is None:\n"
        "        print(f'  absent   {name}')\n"
        "        continue\n"
        "    try:\n"
        "        importlib.import_module(name)\n"
        "        print(f'  ok       {name}')\n"
        "    except Exception as exc:\n"
        "        print(f'  FAILED   {name}: {exc!r}')\n"
        "        failed.append(name)\n"
        "sys.exit(1 if failed else 0)\n"
    )
    print("Checking native extensions in bundled CPython:")
    # Run from a neutral cwd so a stray ``potato`` source dir is not picked up.
    result = subprocess.run(
        [str(python), "-c", code],
        check=False,
        cwd=str(python.parent),
    )
    if result.returncode != 0:
        print(
            "Native extension import check failed in bundled CPython "
            "(missing VC++ runtime DLLs or a broken site install)",
            file=sys.stderr,
        )
        return result.returncode
    return 0


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
    if (
        not (repo / "pyproject.toml").is_file()
        and not (repo / "setup.py").is_file()
    ):
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

    rc = stage_windows_crt(python)
    if rc != 0:
        return rc

    rc = verify_native_extensions(python)
    if rc != 0:
        return rc

    verify = subprocess.run(
        [str(python), "-c", "import potato.tauri.entry"],
        check=False,
        cwd=str(python.parent),
    )
    if verify.returncode != 0:
        print("Potato import check failed after install", file=sys.stderr)
        return verify.returncode
    print("Bundled CPython can import potato.tauri.entry")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
