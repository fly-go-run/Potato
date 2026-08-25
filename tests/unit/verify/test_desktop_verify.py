# -*- coding: utf-8 -*-
from __future__ import annotations

import sys

from scripts.verify import desktop_verify


def test_tauri_without_cdp_keeps_backend_smoke_and_skips_ui(
    monkeypatch,
    capsys,
) -> None:
    checked: list[str] = []
    monkeypatch.setattr(
        desktop_verify,
        "health_check",
        lambda base_url: checked.append(base_url) or "2.0.6",
    )
    monkeypatch.setattr(
        desktop_verify,
        "verify_frontend",
        lambda _base_url: (_ for _ in ()).throw(
            AssertionError("embedded Tauri frontend is not served by backend"),
        ),
    )
    monkeypatch.setattr(
        sys,
        "argv",
        [
            "desktop_verify.py",
            "--base-url",
            "http://127.0.0.1:50816",
            "--ui-mode",
            "tauri-windows",
            "--skip-chat",
        ],
    )

    assert desktop_verify.main() == 0
    assert checked == ["http://127.0.0.1:50816"]
    assert "embedded Tauri UI verification (CDP unavailable)" in (
        capsys.readouterr().out
    )
