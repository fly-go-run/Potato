# -*- coding: utf-8 -*-
from pathlib import Path

import pytest
from fastapi import HTTPException

from potato.app.utils import save_upload_with_limit


class _Upload:
    def __init__(self, chunks: list[bytes]):
        self._chunks = iter(chunks)

    async def read(self, _size: int) -> bytes:
        return next(self._chunks)


@pytest.mark.asyncio
async def test_save_upload_with_limit_streams_to_disk(tmp_path: Path) -> None:
    destination = tmp_path / "upload.bin"

    size = await save_upload_with_limit(
        _Upload([b"abc", b"def", b""]),
        destination,
        default_max_size_mb=1,
    )

    assert size == 6
    assert destination.read_bytes() == b"abcdef"


@pytest.mark.asyncio
async def test_save_upload_with_limit_removes_partial_file_on_overflow(
    tmp_path: Path,
) -> None:
    destination = tmp_path / "upload.bin"

    with pytest.raises(HTTPException) as exc_info:
        await save_upload_with_limit(
            _Upload([b"0123456789", b""]),
            destination,
            default_max_size_mb=0,
        )

    assert exc_info.value.status_code == 413
    assert not destination.exists()
