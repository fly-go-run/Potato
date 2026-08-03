# -*- coding: utf-8 -*-
from pathlib import Path

import pytest

from qwenpaw.agents.tools.browser_paths import (
    browser_type_from_executable,
    resolve_browser_user_data_dir,
    safe_download_filename,
    validate_browser_executable_path,
)


def test_validate_browser_executable_path_accepts_existing_browser(
    tmp_path: Path,
) -> None:
    executable = tmp_path / "Google Chrome"
    executable.touch()

    validate_browser_executable_path(str(executable))


@pytest.mark.parametrize("filename", ["tool", "browser.exe", ""])
def test_validate_browser_executable_path_rejects_untrusted_or_missing_binary(
    tmp_path: Path,
    filename: str,
) -> None:
    if filename == "tool":
        (tmp_path / filename).touch()

    with pytest.raises(ValueError, match="executable_path rejected"):
        validate_browser_executable_path(str(tmp_path / filename))


def test_browser_type_and_profile_directory_preserve_default_profile() -> None:
    assert (
        browser_type_from_executable("/Applications/Chromium.app/Chromium")
        == "chromium"
    )
    assert browser_type_from_executable("/opt/custom-browser") == ""
    assert (
        resolve_browser_user_data_dir("/workspace", "/opt/custom-browser")
        == "/workspace/browser/user_data"
    )
    assert (
        resolve_browser_user_data_dir(
            "/workspace",
            "/Applications/Google Chrome",
            explicit_executable_path=True,
        )
        == "/workspace/browser/user_data_chrome"
    )


@pytest.mark.parametrize(
    ("value", "expected"),
    [
        ("../../report?.pdf", "report_.pdf"),
        (" . ", "download"),
        (None, "download"),
    ],
)
def test_safe_download_filename(value: str | None, expected: str) -> None:
    assert safe_download_filename(value) == expected
