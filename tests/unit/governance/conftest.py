# -*- coding: utf-8 -*-
"""Keep governance unit tests off the real user-global rules file."""
from __future__ import annotations

import pytest

from potato.governance.policy import clear_global_rules_cache


@pytest.fixture(autouse=True)
def _isolate_global_rules(tmp_path, monkeypatch):
    path = tmp_path / "global-governance" / "default.rules.yaml"
    monkeypatch.setattr(
        "potato.governance.global_rules.global_rules_path",
        lambda: path,
    )
    clear_global_rules_cache()
    yield
    clear_global_rules_cache()
