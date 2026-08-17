# -*- coding: utf-8 -*-
from __future__ import annotations

from unittest.mock import patch

import pytest
from fastapi import FastAPI
from fastapi.testclient import TestClient

from potato.app.routers.computer_use import router
from potato.config.config import ComputerUseConfig, Config


@pytest.fixture
def client(tmp_path, monkeypatch: pytest.MonkeyPatch) -> TestClient:
    config = Config()
    config.computer_use = ComputerUseConfig(enabled=False)

    def _load() -> Config:
        return config

    def _save(updated: Config, config_path=None) -> None:
        config.computer_use = updated.computer_use

    monkeypatch.setattr("potato.app.routers.computer_use.load_config", _load)
    monkeypatch.setattr("potato.app.routers.computer_use.save_config", _save)
    monkeypatch.setattr(
        "potato.app.routers.computer_use.resolve_cua_driver_binary",
        lambda _explicit="": "",
    )
    monkeypatch.setattr(
        "potato.app.routers.computer_use.ensure_driver_binary",
        lambda _explicit="": "/tmp/cua-driver",
    )
    app = FastAPI()
    app.include_router(router)
    client = TestClient(app)
    client.config = config  # type: ignore[attr-defined]
    return client


def test_get_and_enable_computer_use(client: TestClient) -> None:
    status = client.get("/computer-use").json()
    assert status["enabled"] is False
    assert status["driver_available"] is False

    updated = client.put("/computer-use", json={"enabled": True}).json()
    assert updated["enabled"] is True

    allowed = client.put(
        "/computer-use",
        json={"always_allowed_apps": ["com.apple.calculator"]},
    ).json()
    assert allowed["always_allowed_apps"] == ["com.apple.calculator"]


def test_put_rejects_driver_path(client: TestClient) -> None:
    response = client.put(
        "/computer-use",
        json={"driver_path": "/tmp/not-a-driver"},
    )
    assert response.status_code == 422


def test_put_rejects_display_name_allow_entry(client: TestClient) -> None:
    response = client.put(
        "/computer-use",
        json={"always_allowed_apps": ["Calculator"]},
    )
    assert response.status_code == 422


def test_put_filters_protected_allow_entry(client: TestClient) -> None:
    response = client.put(
        "/computer-use",
        json={"always_allowed_apps": ["com.apple.Terminal"]},
    )
    assert response.status_code == 200
    assert response.json()["always_allowed_apps"] == []


def test_put_removes_legacy_display_name_from_settings(
    client: TestClient,
) -> None:
    client.config.computer_use.always_allowed_apps = [  # type: ignore[attr-defined]
        "Calculator",
        "com.apple.calculator",
    ]
    response = client.put("/computer-use", json={})
    assert response.status_code == 200
    assert response.json()["always_allowed_apps"] == ["com.apple.calculator"]
    assert client.config.computer_use.always_allowed_apps == [  # type: ignore[attr-defined]
        "com.apple.calculator",
    ]


def test_get_does_not_prepare_driver(
    client: TestClient,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    def _unexpected_prepare(_explicit: str = "") -> str:
        raise AssertionError("GET must not prepare or download the driver")

    monkeypatch.setattr(
        "potato.app.routers.computer_use.ensure_driver_binary",
        _unexpected_prepare,
    )
    response = client.get("/computer-use")
    assert response.status_code == 200
    assert response.json()["driver_available"] is False


def test_enable_fails_closed_when_driver_prepare_fails(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    config = Config()
    config.computer_use = ComputerUseConfig(enabled=False)

    def _load() -> Config:
        return config

    def _save(updated: Config, config_path=None) -> None:
        config.computer_use = updated.computer_use

    monkeypatch.setattr("potato.app.routers.computer_use.load_config", _load)
    monkeypatch.setattr("potato.app.routers.computer_use.save_config", _save)
    monkeypatch.setattr(
        "potato.app.routers.computer_use.resolve_cua_driver_binary",
        lambda _explicit="": "",
    )

    def _fail(_explicit: str = "") -> str:
        raise RuntimeError("hash mismatch")

    monkeypatch.setattr(
        "potato.app.routers.computer_use.ensure_driver_binary",
        _fail,
    )
    app = FastAPI()
    app.include_router(router)
    client = TestClient(app)
    response = client.put("/computer-use", json={"enabled": True})
    assert response.status_code == 400
    assert config.computer_use.enabled is False
