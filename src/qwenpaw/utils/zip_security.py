# -*- coding: utf-8 -*-
"""Shared resource and metadata validation for untrusted ZIP archives."""
from __future__ import annotations

import stat
import zipfile
from dataclasses import dataclass
from pathlib import PurePosixPath, PureWindowsPath


@dataclass(frozen=True)
class ZipLimits:
    max_entries: int
    max_total_uncompressed: int
    max_member_uncompressed: int
    max_compression_ratio: float


MIB = 1024 * 1024

# HTTP imports are read into memory and extracted on a request worker, so keep
# their limits intentionally tighter than locally managed backup archives.
WEB_UPLOAD_ZIP_LIMITS = ZipLimits(
    max_entries=10_000,
    max_total_uncompressed=200 * MIB,
    max_member_uncompressed=100 * MIB,
    max_compression_ratio=1_000.0,
)

# Backups can legitimately contain full workspaces. They still need finite
# limits so a trusted foreign or corrupted archive cannot expand forever.
BACKUP_ZIP_LIMITS = ZipLimits(
    max_entries=250_000,
    max_total_uncompressed=20 * 1024 * MIB,
    # Workspaces may contain media, datasets, or generated artifacts larger
    # than a few megabytes. Keep the per-entry cap finite without making
    # ordinary local backups unusable.
    max_member_uncompressed=1024 * MIB,
    max_compression_ratio=10_000.0,
)


class ZipResourceLimitError(ValueError):
    """Raised when an archive exceeds a resource or metadata limit."""


def normalize_zip_member_name(name: str) -> str:
    """Normalize a ZIP member name before applying a path containment check.

    ZIP archives are portable, but :class:`pathlib.Path` only interprets the
    separators native to the host running the service. Normalize backslashes
    first and reject both POSIX and Windows absolute/drive-qualified names so
    the same archive cannot pass validation on one platform and escape on
    another.
    """
    normalized = str(name).replace("\\", "/")
    if "\x00" in normalized:
        raise ValueError(f"ZIP member contains a NUL byte: {name!r}")
    windows = PureWindowsPath(normalized)
    if (
        PurePosixPath(normalized).is_absolute()
        or windows.is_absolute()
        or bool(windows.drive)
    ):
        raise ValueError(f"Absolute path in ZIP member is not allowed: {name}")
    return normalized


def validate_zip_archive(
    zf: zipfile.ZipFile,
    limits: ZipLimits,
    *,
    reject_symlinks: bool = True,
) -> None:
    """Validate central-directory metadata before reading archive contents."""
    infos = zf.infolist()
    if len(infos) > limits.max_entries:
        raise ZipResourceLimitError(
            f"Zip contains too many entries ({len(infos)} > "
            f"{limits.max_entries})",
        )

    total = 0
    seen_names: set[str] = set()
    for info in infos:
        logical_name = info.filename.replace("\\", "/").casefold()
        if logical_name in seen_names:
            raise ZipResourceLimitError(
                f"Zip contains duplicate path: {info.filename}",
            )
        seen_names.add(logical_name)
        if info.is_dir():
            continue
        if info.flag_bits & 0x1:
            raise ZipResourceLimitError(
                f"Encrypted zip entries are not supported: {info.filename}",
            )
        mode = info.external_attr >> 16
        if reject_symlinks and stat.S_ISLNK(mode):
            raise ZipResourceLimitError(
                f"Symlink entries are not allowed: {info.filename}",
            )
        if info.file_size > limits.max_member_uncompressed:
            raise ZipResourceLimitError(
                f"Zip member is too large: {info.filename}",
            )
        total += info.file_size
        if total > limits.max_total_uncompressed:
            raise ZipResourceLimitError("Zip uncompressed size limit exceeded")

        if info.file_size:
            if info.compress_size <= 0:
                raise ZipResourceLimitError(
                    f"Invalid compressed size for: {info.filename}",
                )
            ratio = info.file_size / info.compress_size
            if ratio > limits.max_compression_ratio:
                raise ZipResourceLimitError(
                    f"Zip member compression ratio is too high: "
                    f"{info.filename}",
                )
