#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""Stage the official cua-driver release into the Tauri resource tree."""

from __future__ import annotations

import argparse
import sys
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parents[2]
SRC = REPO_ROOT / "src"
if str(SRC) not in sys.path:
    sys.path.insert(0, str(SRC))

from potato.computer_use.bundle import (  # noqa: E402
    CUA_DRIVER_VERSION,
    binary_digest_is_pinned,
    cua_driver_download_url,
    cua_driver_platform,
    driver_executable_name,
    extract_official_archive,
    verify_archive_digest,
)
import tempfile
import urllib.request


def _http_get(url: str) -> bytes:
    request = urllib.request.Request(url, headers={"User-Agent": "potato-build"})
    with urllib.request.urlopen(request, timeout=180) as response:
        return response.read()


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument(
        "--dest",
        required=True,
        help="Directory that will contain the cua-driver executable",
    )
    args = parser.parse_args()
    dest_dir = Path(args.dest)
    dest_dir.mkdir(parents=True, exist_ok=True)
    dest_binary = dest_dir / driver_executable_name()
    version_file = dest_dir / "VERSION"
    archive_name = cua_driver_platform()[1]
    if (
        dest_binary.is_file()
        and version_file.is_file()
        and version_file.read_text(encoding="utf-8").strip()
        == CUA_DRIVER_VERSION
        and binary_digest_is_pinned(dest_binary, archive_name)
    ):
        print(f"Reusing staged cua-driver {CUA_DRIVER_VERSION} at {dest_binary}")
        return 0

    url = cua_driver_download_url()
    print(f"Downloading {url}")
    with tempfile.TemporaryDirectory(prefix="potato-stage-cua-") as tmp:
        archive = Path(tmp) / "driver-archive"
        archive.write_bytes(_http_get(url))
        verify_archive_digest(archive, archive_name)
        extract_official_archive(archive, dest_binary)
    version_file.write_text(CUA_DRIVER_VERSION + "\n", encoding="utf-8")
    print(f"Staged {dest_binary}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
