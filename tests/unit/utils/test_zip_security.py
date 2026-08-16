# -*- coding: utf-8 -*-
from __future__ import annotations

import io
import stat
import zipfile

import pytest

from potato.utils.zip_security import (
    ZipLimits,
    ZipResourceLimitError,
    normalize_zip_member_name,
    validate_zip_archive,
)


def _archive(entries: list[tuple[zipfile.ZipInfo | str, bytes]]) -> bytes:
    buf = io.BytesIO()
    with zipfile.ZipFile(buf, "w", zipfile.ZIP_DEFLATED) as zf:
        for name_or_info, content in entries:
            zf.writestr(name_or_info, content)
    return buf.getvalue()


def _limits(**overrides) -> ZipLimits:
    values = {
        "max_entries": 10,
        "max_total_uncompressed": 1_000,
        "max_member_uncompressed": 800,
        "max_compression_ratio": 20.0,
    }
    values.update(overrides)
    return ZipLimits(**values)


def test_accepts_archive_within_limits() -> None:
    with zipfile.ZipFile(io.BytesIO(_archive([("ok.txt", b"hello")]))) as zf:
        validate_zip_archive(zf, _limits())


def test_rejects_too_many_entries() -> None:
    data = _archive([("a", b"1"), ("b", b"2")])
    with zipfile.ZipFile(io.BytesIO(data)) as zf:
        with pytest.raises(ZipResourceLimitError, match="too many entries"):
            validate_zip_archive(zf, _limits(max_entries=1))


def test_rejects_total_uncompressed_size() -> None:
    data = _archive([("a", b"a" * 60), ("b", b"b" * 60)])
    with zipfile.ZipFile(io.BytesIO(data)) as zf:
        with pytest.raises(ZipResourceLimitError, match="size limit"):
            validate_zip_archive(
                zf,
                _limits(max_total_uncompressed=100),
            )


def test_rejects_excessive_compression_ratio() -> None:
    data = _archive([("bomb", b"0" * 10_000)])
    with zipfile.ZipFile(io.BytesIO(data)) as zf:
        with pytest.raises(ZipResourceLimitError, match="compression ratio"):
            validate_zip_archive(
                zf,
                _limits(
                    max_member_uncompressed=20_000,
                    max_total_uncompressed=20_000,
                    max_compression_ratio=2,
                ),
            )


def test_rejects_symlink_entry() -> None:
    info = zipfile.ZipInfo("link")
    info.create_system = 3
    info.external_attr = (stat.S_IFLNK | 0o777) << 16
    data = _archive([(info, b"target")])
    with zipfile.ZipFile(io.BytesIO(data)) as zf:
        with pytest.raises(ZipResourceLimitError, match="Symlink"):
            validate_zip_archive(zf, _limits())


def test_rejects_cross_platform_duplicate_paths() -> None:
    data = _archive([("Dir/File.txt", b"a"), (r"dir\file.txt", b"b")])
    with zipfile.ZipFile(io.BytesIO(data)) as zf:
        with pytest.raises(ZipResourceLimitError, match="duplicate path"):
            validate_zip_archive(zf, _limits())


@pytest.mark.parametrize("name", ["/tmp/out", r"C:\tmp\out", r"..\out"])
def test_normalizes_or_rejects_cross_platform_member_paths(name: str) -> None:
    if name == r"..\out":
        assert normalize_zip_member_name(name) == "../out"
    else:
        with pytest.raises(ValueError, match="Absolute path"):
            normalize_zip_member_name(name)
