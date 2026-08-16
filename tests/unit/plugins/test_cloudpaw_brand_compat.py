# -*- coding: utf-8 -*-
"""Compatibility tests for CloudPaw's external iac-code integration."""

from __future__ import annotations

import sys
from pathlib import Path

import yaml


_BUNDLE_DIR = Path(__file__).resolve().parents[3] / "plugins" / "bundle"
if str(_BUNDLE_DIR) not in sys.path:
    sys.path.insert(0, str(_BUNDLE_DIR))

from cloudpaw import agents_setup  # noqa: E402


def test_iac_code_receives_current_secret_dir(monkeypatch, tmp_path: Path) -> None:
    from potato import constant

    secret_dir = tmp_path / ".potato.secret"
    monkeypatch.setattr(constant, "SECRET_DIR", secret_dir)
    monkeypatch.delenv("IAC_CODE_PROVIDER", raising=False)
    monkeypatch.setattr(
        agents_setup,
        "_write_iac_code_partner_source_to_settings",
        lambda: None,
    )
    env: dict[str, str] = {}

    agents_setup._inject_llm_env(env)

    assert env["QWENPAW_SECRET_DIR"] == str(secret_dir)


def test_iac_code_partner_source_keeps_legacy_protocol(
    tmp_path: Path,
) -> None:
    settings_path = tmp_path / ".iac-code" / "settings.yml"
    settings_path.parent.mkdir(parents=True)
    settings_path.write_text(
        "llm_source: potato\nmodel: existing-model\n",
        encoding="utf-8",
    )

    agents_setup._write_iac_code_partner_source_to_settings(settings_path)

    settings = yaml.safe_load(settings_path.read_text(encoding="utf-8"))
    assert settings == {
        "llm_source": "qwenpaw",
        "model": "existing-model",
    }
